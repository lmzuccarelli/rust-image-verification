use custom_logger::*;
use serde_derive::{Deserialize, Serialize};
use std::env;
use std::fs::{self};
use walkdir::WalkDir;

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

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ManifestPlatform {
    #[serde(rename = "architecture")]
    pub architecture: String,

    #[serde(rename = "os")]
    pub os: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Common {
    #[serde(rename = "name")]
    pub name: String,

    #[serde(rename = "blob")]
    pub blob: String,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // setup logging
    let log = &Logging {
        log_level: Level::INFO,
    };

    let mut component: &str;
    let mut dir: String;

    //  get blobs directory
    let base_dir = &args[1].split("working-dir").nth(0).unwrap();
    let blobs_dir = format!("{}/working-dir/blobs-store/", &base_dir);

    for entry in WalkDir::new(&args[1]).into_iter().filter_map(Result::ok) {
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

            let config_blob_sum = manifest
                .config
                .as_ref()
                .unwrap()
                .digest
                .split(":")
                .collect::<Vec<&str>>()[1];
            let config_size = manifest.config.clone().unwrap().size;

            // check for config size
            let blob_file = format!(
                "{}/{}/{}",
                &blobs_dir,
                &config_blob_sum[..2],
                &config_blob_sum
            );
            let size = fs::metadata(&blob_file).unwrap().len();
            log.hi(&format!("  config  blob {}", &config_blob_sum));
            assert_eq!(size, config_size as u64);

            // iterate through each components related layer
            // and verify size
            for layer in manifest.layers.unwrap().iter() {
                let blob_sum = layer.digest.split(":").collect::<Vec<&str>>()[1];
                let blob_size = layer.size;
                let layer_file = format!("{}/{}/{}", &blobs_dir, &blob_sum[..2], &blob_sum);
                let size = fs::metadata(&layer_file).unwrap().len();
                log.lo(&format!("  related blob {}", &blob_sum));
                assert_eq!(size, blob_size as u64);
            }
        }
    }
}
