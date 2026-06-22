use std::path::Path;

use crate::config::models::{self, ModelAsset};

pub struct ModelManager;

impl ModelManager {
    pub fn ensure_assets(model_name: &str, app_data_dir: &Path) -> bool {
        let assets = models::model_assets();
        let Some(model_assets) = assets.get(model_name) else {
            return true;
        };

        for asset in model_assets {
            let dest_path = app_data_dir.join("models").join(&asset.dest);
            if Self::check_asset_exists(asset, &dest_path) {
                continue;
            }
            match asset.asset_type.as_str() {
                "local" => return false,
                "huggingface" | "git" => {
                    if !Self::download_asset(asset, &dest_path) {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        true
    }

    fn check_asset_exists(asset: &ModelAsset, dest_path: &Path) -> bool {
        if !dest_path.exists() {
            return false;
        }
        if let Some(essential) = &asset.essential_files {
            for file in essential {
                if !dest_path.join(file).exists() {
                    return false;
                }
            }
        }
        true
    }

    fn download_asset(asset: &ModelAsset, dest_path: &Path) -> bool {
        let Some(url) = &asset.url else {
            eprintln!("[ModelManager] no URL for asset {}", asset.name);
            return false;
        };
        eprintln!(
            "[ModelManager] TODO: download {} from {} to {}",
            asset.name,
            url,
            dest_path.display()
        );
        true
    }
}