use clap::Parser;
use custom_logger::*;
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use serde_derive::{Deserialize, Serialize};
use sha256::*;
use std::fs::{self, read};
use std::os::unix::fs::MetadataExt;
use walkdir::WalkDir;

mod api;
use api::schema::*;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsLayer {
    pub blob_sum: String,
    pub original_ref: Option<String>,
    pub size: Option<i64>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub media_type: String,
    pub size: i64,
    pub digest: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: Option<i64>,

    #[serde(rename = "digest")]
    pub digest: Option<String>,

    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,

    #[serde(rename = "platform")]
    pub platform: Option<ManifestPlatform>,

    #[serde(rename = "size")]
    pub size: Option<i64>,

    #[serde(rename = "config")]
    pub config: Option<Layer>,

    #[serde(rename = "layers")]
    pub layers: Option<Vec<Layer>>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestList {
    #[serde(rename = "schemaVersion")]
    pub schema_version: Option<i64>,

    #[serde(rename = "mediaType")]
    pub media_type: Option<String>,

    #[serde(rename = "manifests")]
    pub manifests: Vec<Manifest>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestPlatform {
    #[serde(rename = "architecture")]
    pub architecture: String,

    #[serde(rename = "os")]
    pub os: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Common {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "blob")]
    pub blob: String,
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let base_dir = args.base_dir.to_string();
    let release_dir = args.release_dir;
    let operators_dir = args.operators_dir;
    let level = args.loglevel.unwrap().to_string();

    // convert to enum
    let res_log_level = match level.as_str() {
        "info" => Level::INFO,
        "debug" => Level::DEBUG,
        "trace" => Level::TRACE,
        _ => Level::INFO,
    };

    // setup logging
    let log = &Logging {
        log_level: res_log_level,
    };

    let mut component: &str;
    let mut dir: String;

    //  get blobs directory
    let blobs_dir = format!("{}/blobs-store/", &base_dir);
    let mut futs = FuturesUnordered::new();
    let batch_size = 16;
    let mut blob_sum: String;
    let mut blob_size: i64;
    let mut manifest_config: Layer;
    let mut vec_blobs: Vec<String> = vec![];

    // check release
    if release_dir.is_some() {
        log.hi("verifying release blobs");
        // shadow the release_dir variable
        let release_dir = format!("{}/{}/release/", &base_dir, &release_dir.unwrap());
        for entry in WalkDir::new(&release_dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.path().is_dir() {
                dir = entry.path().display().to_string();
                component = dir.split("/").last().unwrap();
                if !component.starts_with("release") {
                    log.ex(&format!("component: {:#?}", &component));
                }
            }
            if entry.path().is_file() {
                let file_name = "".to_string() + entry.path().display().to_string().as_str();
                let data = fs::read_to_string(&file_name).expect("should read manifest file");
                let manifest: Manifest =
                    serde_json::from_str(&data).expect("should be able to parse manifest");

                manifest_config = manifest.clone().config.unwrap();
                blob_size = manifest_config.clone().size;
                blob_sum = manifest_config.digest.split(":").collect::<Vec<&str>>()[1].to_string();

                // get blobs in batch as set by batch_size
                // each future handles opens and verifies a file
                // with 8 threads (one per digest)
                // batch the calls
                // push the config details first
                futs.push(verify_file(log, blobs_dir.clone(), blob_sum, blob_size));

                let layers = manifest.clone().layers.unwrap();

                // iterate through each components related layer
                // and verify sha256 contents and size
                for layer in layers.iter() {
                    let blob_sum = layer.digest.split(":").collect::<Vec<&str>>()[1];
                    let blob_size = layer.size;
                    // don't re-evaluate duplicates
                    // saves time :)
                    if !vec_blobs.contains(&blob_sum.to_string()) {
                        futs.push(verify_file(
                            log,
                            blobs_dir.clone(),
                            blob_sum.to_string(),
                            blob_size,
                        ));
                        vec_blobs.insert(0, blob_sum.to_string());
                    } else {
                        log.lo(&format!("  blob verified {}", blob_sum));
                    }
                    if futs.len() >= batch_size {
                        let _response = futs.next().await.unwrap();
                    }
                }
                // wait for the remaining to finish.
                while let Some(_response) = futs.next().await {}
            }
        }
    }

    if operators_dir.is_some() {
        log.hi("verifying operator blobs");
        // shadow the operators_dir variable
        let operators_dir = format!("{}/{}/operators/", &base_dir, &operators_dir.unwrap());
        let mut component_full_name: String = "".to_string();
        for entry in WalkDir::new(&operators_dir)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.path().is_dir() {
                dir = entry.path().display().to_string();
                component = dir.split("/").last().unwrap();
                if component.contains("sha256:") {
                    log.ex(&format!("component: {:#?}", &component_full_name));
                    component_full_name = "".to_string();
                } else {
                    if component != "" && component_full_name == "" {
                        component_full_name = component.to_string();
                    }
                    if component != "" && component_full_name != "" {
                        component_full_name = component_full_name + "/" + &component.to_string();
                    }
                }
            }
            if entry.path().is_file() {
                let file_name = "".to_string() + entry.path().display().to_string().as_str();
                if !file_name.contains("manifest-list.json") {
                    let data =
                        fs::read_to_string(file_name.clone()).expect("should read manifest*.json");

                    let manifest: Manifest =
                        serde_json::from_str(&data).expect("should be able to parse manifest");
                    // read all manifests execept manifest-lists
                    if manifest.clone().media_type.is_some()
                        && manifest.clone().media_type.unwrap()
                            != "application/vnd.docker.distribution.manifest.list.v2+json"
                    {
                        manifest_config = manifest.clone().config.unwrap();
                        blob_size = manifest_config.clone().size;
                        blob_sum =
                            manifest_config.digest.split(":").collect::<Vec<&str>>()[1].to_string();

                        // get blobs in batch as set by batch_size
                        // each future handles opens and verifies a file
                        // with 8 threads (one per digest)
                        // batch the calls
                        // push the config details first
                        futs.push(verify_file(log, blobs_dir.clone(), blob_sum, blob_size));

                        let layers = manifest.clone().layers.unwrap();

                        // iterate through each components related layer
                        // and verify sha256 contents and size
                        for layer in layers.iter() {
                            let blob_sum = layer.digest.split(":").collect::<Vec<&str>>()[1];
                            let blob_size = layer.size;
                            // don't re-evaluate duplicates
                            // saves time :)
                            if !vec_blobs.contains(&blob_sum.to_string()) {
                                futs.push(verify_file(
                                    log,
                                    blobs_dir.clone(),
                                    blob_sum.to_string(),
                                    blob_size,
                                ));
                                vec_blobs.insert(0, blob_sum.to_string());
                            } else {
                                log.lo(&format!("  blob verified {}", blob_sum));
                            }
                            if futs.len() >= batch_size {
                                let _response = futs.next().await.unwrap();
                            }
                        }
                        // wait for the remaining to finish.
                        while let Some(_response) = futs.next().await {}
                    }
                }
            }
        }
    }

    // verify_file - function to check size and sha256 hash of contents
    async fn verify_file(log: &Logging, blobs_dir: String, blob_sum: String, blob_size: i64) {
        let file = format!("{}/{}/{}", &blobs_dir, &blob_sum[..2], &blob_sum);
        let res = fs::metadata(&file);
        match res {
            Ok(res) => {
                log.mid(&format!("  related blob  {}", &blob_sum));
                assert_eq!(res.size(), blob_size as u64);
                let bytes = read(&file).unwrap();
                let hash = digest(&bytes);
                assert_eq!(hash, blob_sum);
            }
            Err(_) => {
                log.error(&format!("  blob missing  {}", &blob_sum));
            }
        }
    }
}
