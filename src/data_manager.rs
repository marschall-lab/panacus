use std::{
    collections::{HashMap, HashSet},
    io::{BufWriter, Error, Write},
    str,
};

use abacus::{AbacusAuxilliary, AbacusByTotal};
use graph::GraphAuxilliary;
use strum::IntoEnumIterator;

use crate::{
    analyses::InputRequirement as Req, io::bufreader_from_compressed_gfa, util::CountType,
};

mod abacus;
mod graph;
mod hist;
mod util;

pub use abacus::AbacusByGroup;
pub use abacus::ViewParams;
pub use graph::Edge;
pub use graph::ItemId;
pub use graph::Orientation;
pub use graph::PathSegment;
pub use hist::Hist;
pub use hist::HistAuxilliary;

#[derive(Debug)]
pub struct DataManager {
    // GraphAuxilliary
    graph_aux: Option<GraphAuxilliary>,

    // AbabcusAuxilliary
    abacus_aux_params: ViewParams,
    abacus_aux: Option<AbacusAuxilliary>,

    total_abaci: Option<HashMap<CountType, AbacusByTotal>>,
    group_abacus: Option<AbacusByGroup>,
    hists: Option<HashMap<CountType, Hist>>,

    path_lens: Option<HashMap<PathSegment, (u32, u32)>>,
    gfa_file: String,
    input_requirements: HashSet<Req>,
    count_type: CountType,
}

impl DataManager {
    pub fn from_gfa_with_view(
        gfa_file: &str,
        input_requirements: HashSet<Req>,
        view_params: &ViewParams,
    ) -> Result<Self, Error> {
        let mut dm = Self::from_gfa(&gfa_file, input_requirements);
        if view_params.groupby_sample {
            dm = dm.with_sample_group();
        } else if view_params.groupby_haplotype {
            dm = dm.with_haplo_group();
        } else if view_params.groupby != "" {
            dm = dm.with_group(&view_params.groupby);
        }
        if view_params.positive_list != "" {
            dm = dm.include_coords(&view_params.positive_list);
        }
        if view_params.negative_list != "" {
            dm = dm.exclude_coords(&view_params.negative_list);
        }
        if view_params.order.is_some() {
            log::debug!("Order given");
            dm = dm.with_order(view_params.order.as_ref().unwrap());
        }
        dm.finish()
    }

    pub fn new() -> Self {
        DataManager {
            graph_aux: None,
            abacus_aux_params: ViewParams::default(),
            abacus_aux: None,
            total_abaci: None,
            group_abacus: None,
            hists: None,
            path_lens: None,
            gfa_file: String::new(),
            input_requirements: HashSet::new(),
            count_type: CountType::All,
        }
    }

    pub fn from_gfa(gfa_file: &str, input_requirements: HashSet<Req>) -> Self {
        let count_type = if input_requirements.contains(&Req::Node)
            && input_requirements.contains(&Req::Edge)
            && input_requirements.contains(&Req::Bp)
        {
            CountType::All
        } else if input_requirements.contains(&Req::Node) {
            CountType::Node
        } else if input_requirements.contains(&Req::Bp) {
            CountType::Bp
        } else if input_requirements.contains(&Req::Edge) {
            CountType::Edge
        } else {
            CountType::Node
        };
        let graph_aux = Some(GraphAuxilliary::from_gfa(gfa_file, count_type));
        DataManager {
            graph_aux,
            abacus_aux_params: ViewParams::default(),
            abacus_aux: None,
            total_abaci: None,
            group_abacus: None,
            hists: None,
            path_lens: None,
            gfa_file: gfa_file.to_owned(),
            input_requirements,
            count_type,
        }
    }

    pub fn with_group(mut self, file_name: &str) -> Self {
        self.abacus_aux_params.groupby = file_name.to_owned();
        self
    }

    pub fn with_haplo_group(mut self) -> Self {
        self.abacus_aux_params.groupby_haplotype = true;
        self
    }

    pub fn with_sample_group(mut self) -> Self {
        self.abacus_aux_params.groupby_sample = true;
        self
    }

    pub fn include_coords(mut self, file_name: &str) -> Self {
        self.abacus_aux_params.positive_list = file_name.to_owned();
        self
    }

    pub fn exclude_coords(mut self, file_name: &str) -> Self {
        self.abacus_aux_params.negative_list = file_name.to_owned();
        self
    }

    pub fn with_order(mut self, file_name: &str) -> Self {
        self.abacus_aux_params.order = Some(file_name.to_owned());
        self
    }

    pub fn finish(self) -> Result<Self, Error> {
        let mut dm = self.set_abacus_aux()?.set_abaci_by_total();
        if dm.input_requirements.contains(&Req::Hist) {
            dm = dm.set_hists();
        }
        if dm.input_requirements.contains(&Req::AbacusByGroup) {
            dm = dm.set_abacus_by_group()?;
        }
        Ok(dm)
    }

    pub fn get_degree(&self) -> &Vec<u32> {
        Self::check_and_error(self.graph_aux.as_ref().unwrap().degree.as_ref(), "degree");
        self.graph_aux.as_ref().unwrap().degree.as_ref().unwrap()
    }

    pub fn get_node_lens(&self) -> &Vec<u32> {
        &self.graph_aux.as_ref().unwrap().node_lens
    }

    pub fn get_edges(&self) -> &HashMap<Edge, ItemId> {
        Self::check_and_error(self.graph_aux.as_ref().unwrap().edge2id.as_ref(), "edge2id");
        self.graph_aux.as_ref().unwrap().edge2id.as_ref().unwrap()
    }

    pub fn get_nodes(&self) -> &HashMap<Vec<u8>, ItemId> {
        &self.graph_aux.as_ref().unwrap().node2id
    }

    pub fn get_node_count(&self) -> usize {
        self.graph_aux.as_ref().unwrap().node_count
    }

    pub fn get_edge_count(&self) -> usize {
        self.graph_aux.as_ref().unwrap().edge_count
    }

    pub fn get_group_count(&self) -> usize {
        Self::check_and_error(self.abacus_aux.as_ref(), "abacus_aux -> group_count");
        self.abacus_aux.as_ref().unwrap().count_groups()
    }

    pub fn get_fname(&self) -> String {
        self.gfa_file.to_string()
    }

    pub fn get_groups(&self) -> &HashMap<PathSegment, String> {
        Self::check_and_error(self.abacus_aux.as_ref(), "abacus_aux -> groups");
        &self.abacus_aux.as_ref().unwrap().groups
    }

    pub fn get_path_lens(&self) -> &HashMap<PathSegment, (u32, u32)> {
        Self::check_and_error(self.path_lens.as_ref(), "path_lens");
        self.path_lens.as_ref().unwrap()
    }

    pub fn get_hists(&self) -> &HashMap<CountType, Hist> {
        Self::check_and_error(self.hists.as_ref(), "hists");
        &self.hists.as_ref().unwrap()
    }

    pub fn get_abacus_by_group(&self) -> &AbacusByGroup {
        Self::check_and_error(self.group_abacus.as_ref(), "abacus_by_group");
        &self.group_abacus.as_ref().unwrap()
    }

    pub fn write_abacus_by_group<W: Write>(
        &self,
        total: bool,
        out: &mut BufWriter<W>,
    ) -> Result<(), Error> {
        Self::check_and_error(self.group_abacus.as_ref(), "abacus_by_group");
        self.group_abacus
            .as_ref()
            .unwrap()
            .to_tsv(total, out, &self.graph_aux.as_ref().unwrap())
    }

    fn set_abacus_aux(mut self) -> Result<Self, Error> {
        self.abacus_aux = Some(AbacusAuxilliary::from_datamgr(
            &self.abacus_aux_params,
            &self.graph_aux.as_ref().unwrap(),
        )?);
        Ok(self)
    }

    fn set_hists(mut self) -> Self {
        let mut hists = HashMap::new();
        for (k, v) in self.total_abaci.as_ref().unwrap() {
            hists.insert(
                *k,
                Hist::from_abacus(v, Some(&self.graph_aux.as_ref().unwrap())),
            );
        }
        self.hists = Some(hists);
        self
    }

    fn check_and_error<T>(value: Option<T>, type_of_value: &str) {
        if value.is_none() {
            let msg = format!(
                "Cannot give value of {}, since it was not requested",
                type_of_value
            );
            log::error!("{}", &msg);
        }
    }

    fn set_abacus_by_group(mut self) -> Result<Self, Error> {
        // let mut abaci_by_group = HashMap::new();
        let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
        let abacus = AbacusByGroup::from_gfa(
            &mut data,
            self.abacus_aux.as_ref().unwrap(),
            &self.graph_aux.as_ref().unwrap(),
            self.count_type,
            true,
        )?;
        // abaci_by_group.insert(self.count_type, abacus);
        self.group_abacus = Some(abacus);
        Ok(self)
    }

    fn set_abaci_by_total(mut self) -> Self {
        let mut abaci = HashMap::new();
        if let CountType::All = self.count_type {
            for count_type in CountType::iter() {
                if let CountType::All = count_type {
                } else {
                    let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
                    let (abacus, path_lens) = AbacusByTotal::from_gfa(
                        &mut data,
                        self.abacus_aux.as_ref().unwrap(),
                        &self.graph_aux.as_ref().unwrap(),
                        count_type,
                    );
                    if count_type == CountType::Node
                        && self.input_requirements.contains(&Req::PathLens)
                    {
                        self.path_lens = Some(path_lens);
                    }
                    abaci.insert(count_type, abacus);
                }
            }
        } else {
            let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
            let (abacus, path_lens) = AbacusByTotal::from_gfa(
                &mut data,
                self.abacus_aux.as_ref().unwrap(),
                &self.graph_aux.as_ref().unwrap(),
                self.count_type,
            );
            if self.input_requirements.contains(&Req::PathLens) {
                self.path_lens = Some(path_lens);
            }
            abaci.insert(self.count_type, abacus);
        }
        self.total_abaci = Some(abaci);
        self
    }
}
