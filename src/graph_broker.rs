use core::panic;
use std::iter::zip;
use std::{
    collections::{HashMap, HashSet},
    io::{BufWriter, Error, Write},
    str,
};

use abacus::{AbacusByTotal, GraphMask};
use graph::GraphStorage;

use crate::{
    analyses::InputRequirement as Req, analysis_parameter::Grouping,
    io::bufreader_from_compressed_gfa, util::CountType,
};

mod abacus;
mod graph;
mod hist;
mod util;

pub use abacus::AbacusByGroup;
pub use abacus::GraphMaskParameters;
pub use graph::Edge;
pub use graph::ItemId;
pub use graph::Orientation;
pub use graph::PathSegment;
pub use hist::Hist;
pub use hist::ThresholdContainer;

#[derive(Debug, Clone)]
pub struct GraphBroker {
    // GraphStorage
    graph_aux: Option<GraphStorage>,

    // AbabcusAuxilliary
    abacus_aux_params: GraphMaskParameters,
    abacus_aux: Option<GraphMask>,

    total_abaci: Option<HashMap<CountType, AbacusByTotal>>,
    group_abacus: Option<AbacusByGroup>,
    hists: Option<HashMap<CountType, Hist>>,
    csc_abacus: bool,

    path_lens: Option<HashMap<PathSegment, (u32, u32)>>,
    gfa_file: String,
    _nice: bool,
    input_requirements: HashSet<Req>,
    count_type: CountType,
}

impl GraphBroker {
    pub fn new() -> Self {
        GraphBroker {
            graph_aux: None,
            abacus_aux_params: GraphMaskParameters::default(),
            abacus_aux: None,
            total_abaci: None,
            group_abacus: None,
            hists: None,
            _nice: false,
            path_lens: None,
            gfa_file: String::new(),
            input_requirements: HashSet::new(),
            count_type: CountType::All,
            csc_abacus: false,
        }
    }

    // TODO: fix situation instead of calculating the third value unnecessary
    fn contains_at_least_two(input_requirements: &HashSet<Req>) -> bool {
        if input_requirements.contains(&Req::Node) && input_requirements.contains(&Req::Edge) {
            return true;
        } else if input_requirements.contains(&Req::Edge) && input_requirements.contains(&Req::Bp) {
            return true;
        } else if input_requirements.contains(&Req::Bp) && input_requirements.contains(&Req::Node) {
            return true;
        } else {
            return false;
        }
    }

    pub fn from_gfa(input_requirements: &HashSet<Req>, nice: bool) -> Self {
        let count_type = if Self::contains_at_least_two(input_requirements) {
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
        let gfa_file = input_requirements
            .iter()
            .find(|v| matches!(v, Req::Graph(_)))
            .expect("Requirements contain gfa file");
        let gfa_file = match gfa_file {
            Req::Graph(gfa_file) => gfa_file,
            _ => panic!("Requirements really need to contain gfa file"),
        };
        let graph_aux = Some(GraphStorage::from_gfa(gfa_file, nice, count_type));
        GraphBroker {
            graph_aux,
            abacus_aux_params: GraphMaskParameters::default(),
            abacus_aux: None,
            total_abaci: None,
            group_abacus: None,
            hists: None,
            path_lens: None,
            gfa_file: gfa_file.to_owned(),
            _nice: nice,
            input_requirements: input_requirements.clone(),
            count_type,
            csc_abacus: false,
        }
    }

    pub fn with_group(self, grouping: &Option<Grouping>) -> Self {
        if let Some(grouping) = grouping {
            match grouping {
                Grouping::Sample => self.with_sample_group(),
                Grouping::Haplotype => self.with_haplo_group(),
                Grouping::Custom(file_name) => self.with_custom_group(file_name),
            }
        } else {
            self
        }
    }

    pub fn with_custom_group(mut self, file_name: &str) -> Self {
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

    pub fn exclude_coords(mut self, exclude: &str) -> Self {
        self.abacus_aux_params.negative_list = exclude.to_owned();
        self
    }

    pub fn with_order(mut self, file_name: &str) -> Self {
        self.abacus_aux_params.order = Some(file_name.to_owned());
        self
    }

    pub fn with_csc_abacus(mut self) -> Self {
        self.csc_abacus = true;
        self
    }

    pub fn finish(self) -> Result<Self, Error> {
        let mut gb = self.set_abacus_aux()?.set_abaci_by_total();
        if gb.input_requirements.contains(&Req::Hist) {
            gb = gb.set_hists();
        }
        let mut has_already_used_abacus = false;
        for req in gb.input_requirements.clone() {
            match req {
                Req::AbacusByGroup(count) => {
                    if has_already_used_abacus {
                        panic!("Panacus is currently not able to have multiple Abaci By Group for different countables. Please run panacus either multiple times or wait for the planned pipelining feature");
                    }
                    if gb.csc_abacus {
                        gb = gb.set_abacus_by_group_csc(count)?;
                    } else {
                        gb = gb.set_abacus_by_group(count)?;
                    }
                    has_already_used_abacus = true;
                }
                _ => continue,
            }
        }
        Ok(gb)
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

    pub fn get_nodes(&self) -> Vec<ItemId> {
        self.graph_aux.as_ref().unwrap().get_nodes()
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
        self.hists.as_ref().unwrap()
    }

    pub fn get_abacus_by_group(&self) -> &AbacusByGroup {
        Self::check_and_error(self.group_abacus.as_ref(), "abacus_by_group");
        self.group_abacus.as_ref().unwrap()
    }

    pub fn get_abacus_by_total(&self, count: CountType) -> &AbacusByTotal {
        Self::check_and_error(self.total_abaci.as_ref(), "abacus_by_group");
        &self.total_abaci.as_ref().unwrap()[&count]
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
            .to_tsv(total, out, self.graph_aux.as_ref().unwrap())
    }

    fn set_abacus_aux(mut self) -> Result<Self, Error> {
        self.abacus_aux = Some(GraphMask::from_datamgr(
            &self.abacus_aux_params,
            self.graph_aux.as_ref().unwrap(),
        )?);
        Ok(self)
    }

    fn set_hists(mut self) -> Self {
        let mut hists = HashMap::new();
        for (k, v) in self.total_abaci.as_ref().unwrap() {
            hists.insert(
                *k,
                Hist::from_abacus(v, Some(self.graph_aux.as_ref().unwrap())),
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

    fn set_abacus_by_group_csc(self, count: CountType) -> Result<Self, Error> {
        let mut res = self.set_abacus_by_group(count)?;
        res.group_abacus.as_mut().unwrap().to_csc();
        Ok(res)
    }

    fn set_abacus_by_group(mut self, count: CountType) -> Result<Self, Error> {
        // let mut abaci_by_group = HashMap::new();
        let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
        let abacus = AbacusByGroup::from_gfa(
            &mut data,
            self.abacus_aux.as_ref().unwrap(),
            self.graph_aux.as_ref().unwrap(),
            count,
            true,
        )?;
        // abaci_by_group.insert(self.count_type, abacus);
        self.group_abacus = Some(abacus);
        Ok(self)
    }

    fn set_abaci_by_total(mut self) -> Self {
        let count_types_not_edge = if self.count_type == CountType::All {
            vec![CountType::Node, CountType::Bp]
        } else if self.count_type != CountType::Edge {
            vec![self.count_type.clone()]
        } else {
            Vec::new()
        };
        let shall_calculate_edge =
            self.count_type == CountType::All || self.count_type == CountType::Edge;
        log::info!(
            "calculating abaci for count_types: {:?}, and edge: {}",
            count_types_not_edge,
            shall_calculate_edge
        );
        let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
        let mut abaci = if !count_types_not_edge.is_empty() {
            let (abaci, path_lens) = AbacusByTotal::from_gfa_multiple(
                &mut data,
                self.abacus_aux.as_ref().unwrap(),
                self.graph_aux.as_ref().unwrap(),
                &count_types_not_edge,
            );
            let abaci: HashMap<CountType, AbacusByTotal> =
                zip(count_types_not_edge, abaci).collect();
            if self.input_requirements.contains(&Req::PathLens) {
                self.path_lens = Some(path_lens);
            }
            abaci
        } else {
            HashMap::new()
        };
        if shall_calculate_edge {
            let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
            let (mut edge_abacus, _) = AbacusByTotal::from_gfa_multiple(
                &mut data,
                self.abacus_aux.as_ref().unwrap(),
                self.graph_aux.as_ref().unwrap(),
                &vec![CountType::Edge],
            );
            abaci.insert(CountType::Edge, edge_abacus.pop().unwrap());
        }
        //if let CountType::All = self.count_type {
        //    for count_type in CountType::iter() {
        //        if let CountType::All = count_type {
        //        } else {
        //            let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
        //            let (abacus, path_lens) = AbacusByTotal::from_gfa(
        //                &mut data,
        //                self.abacus_aux.as_ref().unwrap(),
        //                self.graph_aux.as_ref().unwrap(),
        //                count_type,
        //            );
        //            if count_type == CountType::Node
        //                && self.input_requirements.contains(&Req::PathLens)
        //            {
        //                self.path_lens = Some(path_lens);
        //            }
        //            abaci.insert(count_type, abacus);
        //        }
        //    }
        //} else {
        //    let mut data = bufreader_from_compressed_gfa(&self.gfa_file);
        //    let (abacus, path_lens) = AbacusByTotal::from_gfa(
        //        &mut data,
        //        self.abacus_aux.as_ref().unwrap(),
        //        self.graph_aux.as_ref().unwrap(),
        //        self.count_type,
        //    );
        //    if self.input_requirements.contains(&Req::PathLens) {
        //        self.path_lens = Some(path_lens);
        //    }
        //    abaci.insert(self.count_type, abacus);
        //}
        self.total_abaci = Some(abaci);
        self
    }
}
