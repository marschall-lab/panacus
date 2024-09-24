# Panacus test
#--------------------------------------------------------------------------------
library(stringr)
#Change this path of an old build of panacus
BIN_PREVIOUS_RELEASE_PANACUS="path/to/older/version/bin/panacus"
#The new version is whatever is linked against `panacus` in the terminal
folder_out = ".result"
dir.create(folder_out, showWarnings=F,recursive=T)

### Datasets
#--------------------------------------------------------------------------------
gfa_chrM =  list(file="chrM_test.gfa", short="chrM")
# this needs to be downloaded
#gfa_chr22 = list(file="chr22.hprc-v1.0-pggb.gfa.gz", short="chr22")

### General functions 
#--------------------------------------------------------------------------------
check_res = function(out_rel, out_dev) {
    res_release = read.csv(out_rel, sep="\t",skip=1)
    res_dev = read.csv(out_dev, sep="\t",skip=1)
    all(res_release[4:nrow(res_release),] == res_dev[4:nrow(res_release),])
}

# version at the moment is always the same since
# the release. So no point in using it to distinguish
#get_version = fucntion(cmd) {
#    res = run(paste(cmd, "--version"), intern=T)
#    strsplit(res, split=" ")[[1]][2]
#}

run = function(cmd, ...) {
    args <- list(...)
    intern <- FALSE
    if ("intern" %in% names(args)) {
        intern <- args[["intern"]]
        args <- args[!names(args) %in% "intern"]
    }
    for (i in seq_along(args)) {
        cmd <- sub(paste0("\\{", i, "\\}"), args[[i]], cmd)
    }
  
    cat(cmd,"\n")
    system(cmd, intern = intern)
}

params_to_name = function(params) {
    out = stringr::str_replace_all(params, "-","_")
    out = stringr::str_replace_all(out, ",","-")
    out = stringr::str_replace_all(out, " ","")
    out
}

histgrowth_release = function(gfa, params=NULL) {
    cmd = BIN_PREVIOUS_RELEASE_PANACUS
    #version = get_version(cmd)
    cmd = paste("RUST_LOG=info", cmd, "histgrowth", gfa$file)
    #out = paste0(version,out)
    out = file.path(folder_out, paste0(gfa$short,"_hg", params_to_name(params), "_rel"))
    run(paste(cmd, params, ">", out))
    out
}

histgrowth_dev = function(gfa, params=NULL) {
    cmd = "panacus"
    #version = get_version(cmd)
    cmd = paste("RUST_LOG=info", cmd, "histgrowth", gfa$file)
    out = file.path(folder_out, paste0(gfa$short, "_hg", params_to_name(params), "_dev"))
    #out = paste0(version,out)
    run(paste(cmd, params, ">", out))
    out
}

### Bench ChrM
#--------------------------------------------------------------------------------
##Nodes 
# Sample -S
params=c("-S -q 0,0.5,1.0 -l 0,1,2")
out_rel = histgrowth_release(gfa_chrM, params)
out_dev = histgrowth_dev(gfa_chrM, params)    
check_res(out_rel, out_dev)

# Haplotype -H
params=c("-H -q 0,0.5,1.0 -l 0,1,2")
out_rel = histgrowth_release(gfa_chrM, params) 
out_dev = histgrowth_dev(gfa_chrM, params) 
check_res(out_rel, out_dev)

##Edges
# Sample -S
params=c("-c edge -S -q 0,0.5,1.0 -l 0,1,2")
out_rel = histgrowth_release(gfa_chrM, params)        
out_dev = histgrowth_dev(gfa_chrM, params)      
check_res(out_rel, out_dev)
# Haplotype -H
params=c("-c edge -H -q 0,0.5,1.0 -l 0,1,2")
out_rel = histgrowth_release(gfa_chrM, params)        
out_dev = histgrowth_dev(gfa_chrM, params)      
check_res(out_rel, out_dev)

### Bench Chr22
#--------------------------------------------------------------------------------
##Nodes 
# Sample -S
params=c("-S -q 0,0.5,1.0 -l 0,1,2")
out_rel = histgrowth_release(gfa_chr22, params) #time: ~17s
out_dev = histgrowth_dev(gfa_chr22, params) #time: ~17s
check_res(out_rel, out_dev)

# Haplotype -H
params=c("-H -q 0,0.5,1.0 -l 0,1,2")
out_rel = histgrowth_release(gfa_chr22, params)
out_dev = histgrowth_dev(gfa_chr22, params)
check_res(out_rel, out_dev)

##Edges
# Sample -S
params=c("-c edge -S -q 0,0.5,1.0 -l 0,1,2")
out_rel = histgrowth_release(gfa_chr22, params) #time: ~79s       
out_dev = histgrowth_dev(gfa_chr22, params) #ŧime: ~79s
check_res(out_rel, out_dev)
# Haplotype -H
params=c("-c edge -H -q 0,0.5,1.0 -l 0,1,2")
out_rel = histgrowth_release(gfa_chr22, params)        
out_dev = histgrowth_dev(gfa_chr22, params)      
check_res(out_rel, out_dev)
