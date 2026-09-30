//! System probe of the host.
//!
//! The probe reads the processor, memory, operating system, disks, and the
//! devices the daemon can use. The recommendation then picks the quality
//! that fits the machine. The settings page draws the profile and uses the
//! recommendation to place the two linked sliders.

use alice_core::config::RouterConfig;
use alice_core::dto::{
    RouterPresetDto, RouterStatusDto, SystemCpuCoreDto, SystemCpuDto, SystemDevicesDto,
    SystemDiskDto, SystemGpuDto, SystemMemoryDto, SystemOsDto, SystemProfileDto,
    SystemRecommendationDto, SystemResolverDto,
};
use sysinfo::{Disks, System};

use crate::resolver::device::{
    active_device, available_devices, cuda_available, cuda_build, gpu_available, gpu_build,
};

pub mod gpu;
pub mod preset;

use preset::{
    preset_label, quality_to_router, quality_to_speed, recommend_quality, RecommendationInputs,
};

/// Build the system profile of the host.
pub async fn probe(
    stores: &crate::server::state::AppStores,
    _config: &alice_core::config::CoreConfig,
    router_defaults: &RouterConfig,
) -> SystemProfileDto {
    // 1. System probe
    let mut sys = System::new_all();
    sys.refresh_all();

    // 2. OS
    let os = SystemOsDto {
        name: System::name(),
        version: System::os_version(),
        arch: std::env::consts::ARCH.to_string(),
        kernel_version: System::kernel_version(),
        hostname: System::host_name(),
    };

    // 3. CPU
    let cpus = sys.cpus();
    let logical_cores = cpus.len();
    let physical_cores = sys.physical_core_count();
    let frequency_mhz = cpus.first().map(|cpu| cpu.frequency()).unwrap_or(0);
    let brand = cpus
        .first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let vendor = cpus
        .first()
        .map(|cpu| cpu.vendor_id().to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let usage_percent = cpus
        .iter()
        .map(|cpu| f64::from(cpu.cpu_usage()))
        .sum::<f64>()
        / f64::from(logical_cores.max(1) as u32);
    let cores: Vec<SystemCpuCoreDto> = cpus
        .iter()
        .map(|cpu| SystemCpuCoreDto {
            name: cpu.name().to_string(),
            brand: cpu.brand().to_string(),
            vendor: cpu.vendor_id().to_string(),
            frequency_mhz: cpu.frequency(),
            usage_percent: cpu.cpu_usage(),
        })
        .collect();
    let cpu = SystemCpuDto {
        brand: brand.clone(),
        vendor,
        physical_cores,
        logical_cores,
        frequency_mhz,
        usage_percent: usage_percent as f32,
        cores,
    };

    // 4. Memory - sysinfo already returns bytes on this platform, but the
    // API reports KiB on some versions, so we keep the value as is.
    // In sysinfo 0.33 the values are bytes, so do not multiply.
    let memory = SystemMemoryDto {
        total_bytes: sys.total_memory(),
        available_bytes: sys.available_memory(),
        used_bytes: sys.used_memory(),
        total_swap_bytes: sys.total_swap(),
        used_swap_bytes: sys.used_swap(),
    };

    // 5. Disks
    let disks_sys = Disks::new_with_refreshed_list();
    let disks: Vec<SystemDiskDto> = disks_sys
        .list()
        .iter()
        .map(|disk| SystemDiskDto {
            name: disk.name().to_string_lossy().to_string(),
            mount_point: disk.mount_point().to_string_lossy().to_string(),
            file_system: disk.file_system().to_string_lossy().to_string(),
            total_bytes: disk.total_space(),
            available_bytes: disk.available_space(),
            is_removable: disk.is_removable(),
        })
        .collect();

    // 6. Devices
    let devices = devices_of(stores).await;

    // 7. Resolver state
    let resolver = resolver_of(stores).await;

    // 8. Disk free for the model directory
    let disk_available_bytes = disk_free_for(&disks, &router_defaults.models_dir)
        .or_else(|| disks.first().map(|disk| disk.available_bytes));
    let disk_total_bytes = disks
        .iter()
        .find(|disk| {
            router_defaults
                .models_dir
                .to_string_lossy()
                .starts_with(&disk.mount_point)
        })
        .map(|disk| disk.total_bytes)
        .or_else(|| disks.first().map(|disk| disk.total_bytes));

    // 9. Recommendation inputs - every variable lerps to the quality.
    // Treat any GPU (CUDA or OpenVINO) as CUDA for the recommendation.
    let any_gpu_available = devices.cuda_available || gpu_available();
    let any_gpu_build = devices.cuda_build || gpu_build();
    let inputs = RecommendationInputs {
        total_ram_bytes: memory.total_bytes,
        available_ram_bytes: memory.available_bytes,
        used_ram_bytes: memory.used_bytes,
        logical_cores,
        physical_cores: physical_cores.unwrap_or(0),
        frequency_mhz,
        cpu_usage_percent: usage_percent as f32,
        cuda_available: any_gpu_available,
        cuda_build: any_gpu_build,
        llama_reachable: resolver.llama_reachable,
        embeddings_reachable: resolver.embeddings_reachable,
        gliner_installed: resolver.gliner_installed,
        embedding_installed: resolver.local_embedding_installed,
        reranker_installed: resolver.local_reranker_installed,
        label_count: resolver.label_count,
        intent_count: resolver.intent_count,
        disk_available_bytes,
        disk_total_bytes,
    };
    let (quality, factors, reason) = recommend_quality(&inputs);

    // 10. Router presets
    let recommendation_router = quality_to_router(quality, router_defaults);
    let recommendation_status = to_status(&recommendation_router, stores).await;
    let recommendation = SystemRecommendationDto {
        quality,
        speed: quality_to_speed(quality),
        reason,
        factors,
        router: recommendation_status,
    };

    let presets = presets_of(router_defaults, stores).await;

    SystemProfileDto {
        os,
        cpu,
        memory,
        disks,
        devices,
        resolver,
        recommendation,
        presets,
    }
}

/// Build the device summary.
async fn devices_of(stores: &crate::server::state::AppStores) -> SystemDevicesDto {
    let available = available_devices()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let active = active_device(alice_core::config::LocalDevice::Auto)
        .map(str::to_string)
        .unwrap_or_else(|_| "cpu".to_string());
    // Router and GLiNER share the same device probe; read stored preference when possible.
    let stored = stores
        .settings
        .get_settings()
        .await
        .map(|values| values.router.local_device)
        .unwrap_or(alice_core::config::LocalDevice::Auto);
    let active_local = active_device(stored)
        .map(str::to_string)
        .unwrap_or(active.clone());

    let gliner_installed = stores
        .gliner_store
        .list()
        .iter()
        .filter(|model| model.installed)
        .count();
    let router_installed = stores
        .local_store
        .list()
        .iter()
        .filter(|model| model.installed)
        .count();

    SystemDevicesDto {
        cuda_build: cuda_build(),
        cuda_available: cuda_available(),
        available_devices: available,
        active_device: active,
        active_local_device: active_local,
        gliner_models_installed: gliner_installed,
        router_models_installed: router_installed,
        gpus: graphics_devices().await,
    }
}

/// Read the graphics devices of the host with their memory.
///
/// The probe asks the tools of the machine and reads the files of the
/// kernel, so it runs on the blocking pool of the runtime and never stalls
/// the socket stream.
async fn graphics_devices() -> Vec<SystemGpuDto> {
    let read = tokio::task::spawn_blocking(gpu::gpus)
        .await
        .unwrap_or_default();
    read.into_iter()
        .map(|device| SystemGpuDto {
            name: device.name,
            vendor: device.vendor,
            shared_memory: device.shared_memory,
            memory_total_bytes: device.memory_total_bytes,
            memory_used_bytes: device.memory_used_bytes,
            memory_free_bytes: device.memory_free_bytes,
            driver_version: device.driver_version,
        })
        .collect()
}

/// Build the resolver summary.
async fn resolver_of(stores: &crate::server::state::AppStores) -> SystemResolverDto {
    // Cheap probe without network: read stored values and whether files are on disk.
    let settings = stores.settings.get_settings().await;
    let (base_url, embed_model, router) = match &settings {
        Ok(values) => (
            values.resolver_base_url.clone(),
            values.router.embed_model.clone(),
            values.router.clone(),
        ),
        Err(_) => {
            let defaults = alice_core::config::RouterConfig::default();
            (
                "http://127.0.0.1:8012/v1".to_string(),
                defaults.embed_model.clone(),
                defaults,
            )
        }
    };

    // Try to reach the servers with a short timeout, but do not fail the whole profile.
    let llama_reachable = probe_llama(stores, &base_url).await;
    let llama_detail = if llama_reachable {
        None
    } else {
        Some("Model server does not answer at ".to_string() + &base_url)
    };
    let embeddings_reachable = probe_embeddings(stores, &base_url, &embed_model, &router)
        .await
        .0;
    let embeddings_detail = if embeddings_reachable {
        None
    } else if router.retrieve.uses_embeddings() || router.list_match.uses_embeddings() {
        Some("Embeddings do not answer".to_string())
    } else {
        Some("Router reads words only".to_string())
    };

    let gliner_spec = crate::resolver::gliner::catalog::find(
        settings
            .as_ref()
            .map(|values| values.gliner_model.as_str())
            .unwrap_or("gliner_small-v2.1"),
    )
    .unwrap_or_else(crate::resolver::gliner::catalog::default_spec);

    let gliner_installed = stores.gliner_store.is_installed(gliner_spec);

    let embedding_installed = crate::resolver::local::catalog::find_role(
        settings
            .as_ref()
            .map(|values| values.router.embed_local_model.as_str())
            .unwrap_or("bge-small-en-v1.5"),
        crate::resolver::local::catalog::Role::Embedding,
    )
    .is_some_and(|spec| stores.local_store.is_installed(spec));

    let reranker_installed = crate::resolver::local::catalog::find_role(
        settings
            .as_ref()
            .map(|values| values.router.rerank_model.as_str())
            .unwrap_or("ms-marco-MiniLM-L-6-v2"),
        crate::resolver::local::catalog::Role::Reranker,
    )
    .is_some_and(|spec| stores.local_store.is_installed(spec));

    let intents = stores.intents.list_intents().await.unwrap_or_default();
    let label_count = crate::resolver::gliner::scoring::label_count(&intents);

    SystemResolverDto {
        llama_reachable,
        llama_detail,
        embeddings_reachable,
        embeddings_detail,
        gliner_installed,
        local_embedding_installed: embedding_installed,
        local_reranker_installed: reranker_installed,
        intent_count: intents.len(),
        label_count,
    }
}

/// Probe the llama server with a short timeout.
async fn probe_llama(stores: &crate::server::state::AppStores, base_url: &str) -> bool {
    stores
        .resolver
        .client()
        .list_models(base_url, std::time::Duration::from_secs(2))
        .await
        .is_ok()
}

/// Probe embeddings: server or local file.
async fn probe_embeddings(
    stores: &crate::server::state::AppStores,
    base_url: &str,
    embed_model: &str,
    router: &RouterConfig,
) -> (bool, Option<String>) {
    if !(router.retrieve.uses_embeddings() || router.list_match.uses_embeddings()) {
        return (false, Some("Router reads words only".to_string()));
    }
    if matches!(router.embed_source, alice_core::config::EmbedSource::Local) {
        let spec = crate::resolver::local::catalog::find_role(
            &router.embed_local_model,
            crate::resolver::local::catalog::Role::Embedding,
        );
        return match spec {
            Some(spec) if stores.local_store.is_installed(spec) => (true, None),
            Some(spec) => (false, Some(format!("{} is not on disk", spec.id))),
            None => (false, Some("Unknown embedding model".to_string())),
        };
    }
    let input = vec!["alice".to_string()];
    match stores
        .resolver
        .client()
        .embed(
            base_url,
            embed_model,
            &input,
            std::time::Duration::from_secs(2),
        )
        .await
    {
        Ok(vectors) if !vectors.is_empty() => (true, None),
        Ok(_) => (false, Some("No vector".to_string())),
        Err(err) => (false, Some(err.to_string())),
    }
}

/// Map a router config to the status DTO the frontend shows.
async fn to_status(
    router: &RouterConfig,
    stores: &crate::server::state::AppStores,
) -> RouterStatusDto {
    let local_store = stores.local_store.as_ref();
    let active_local_device = crate::resolver::device::active_device(router.local_device)
        .unwrap_or(crate::resolver::device::DEVICE_CPU)
        .to_string();

    let (embeddings_reachable, embeddings_detail) =
        if router.retrieve.uses_embeddings() || router.list_match.uses_embeddings() {
            // Reuse the same logic as the full status: probe only when needed.
            let settings_probe = stores.settings.get_settings().await.ok();
            let base_url = settings_probe
                .as_ref()
                .map(|values| values.resolver_base_url.as_str())
                .unwrap_or("http://127.0.0.1:8012/v1");
            let embed_model = router.embed_model.as_str();
            if matches!(router.embed_source, alice_core::config::EmbedSource::Local) {
                let spec = crate::resolver::local::catalog::find_role(
                    &router.embed_local_model,
                    crate::resolver::local::catalog::Role::Embedding,
                );
                match spec {
                    Some(spec) if local_store.is_installed(spec) => (true, None),
                    Some(spec) => (false, Some(format!("{} is not on disk", spec.id))),
                    None => (false, Some("Unknown model".to_string())),
                }
            } else {
                match stores
                    .resolver
                    .client()
                    .embed(
                        base_url,
                        embed_model,
                        &["alice".to_string()],
                        std::time::Duration::from_secs(2),
                    )
                    .await
                {
                    Ok(vectors) if !vectors.is_empty() => (true, None),
                    Ok(_) => (false, Some("No vector".to_string())),
                    Err(err) => (false, Some(err.to_string())),
                }
            }
        } else {
            (
                false,
                Some("The router reads the words of the catalog alone.".to_string()),
            )
        };

    RouterStatusDto {
        fast_path: router.fast_path,
        retrieve: router.retrieve.as_str().to_string(),
        decide: router.decide.as_str().to_string(),
        extract: router.extract.as_str().to_string(),
        top_k: router.top_k,
        floor: router.floor,
        margin: router.margin,
        lexical_weight: router.lexical_weight,
        dense_weight: router.dense_weight,
        embed_model: router.embed_model.clone(),
        embed_source: router.embed_source.as_str().to_string(),
        embed_local_model: router.embed_local_model.clone(),
        rerank_model: router.rerank_model.clone(),
        laya_model: router.laya_model.clone(),
        local_device: router.local_device.as_str().to_string(),
        active_local_device,
        local: alice_core::dto::LocalStoreDto {
            models_dir: local_store.root().display().to_string(),
            models: local_store.list(),
            download: local_store.download_state().map(|state| state.to_dto()),
        },
        phrase_gate: router.phrase_gate,
        list_match: router.list_match.as_str().to_string(),
        list_floor: router.list_floor,
        embeddings_reachable,
        embeddings_detail,
    }
}

/// Build the preset list from fastest to best.
async fn presets_of(
    defaults: &RouterConfig,
    stores: &crate::server::state::AppStores,
) -> Vec<RouterPresetDto> {
    let qualities = [10_u8, 30, 50, 75, 95];
    let mut presets = Vec::with_capacity(qualities.len());
    for quality in qualities {
        let router = quality_to_router(quality, defaults);
        let status = to_status(&router, stores).await;
        let (label, description) = preset_label(quality);
        presets.push(RouterPresetDto {
            quality,
            speed: quality_to_speed(quality),
            label: label.to_string(),
            description: description.to_string(),
            router: status,
        });
    }
    presets
}

/// Find the available space of the disk that holds the given path.
fn disk_free_for(disks: &[SystemDiskDto], path: &std::path::Path) -> Option<u64> {
    let path_str = path.to_string_lossy();
    let mut best: Option<&SystemDiskDto> = None;
    for disk in disks {
        if path_str.starts_with(&disk.mount_point)
            && best.is_none_or(|current: &SystemDiskDto| {
                disk.mount_point.len() > current.mount_point.len()
            })
        {
            best = Some(disk);
        }
    }
    best.map(|disk| disk.available_bytes)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn disk_free_picks_the_longest_mount_point() {
        let disks = vec![
            SystemDiskDto {
                name: "/dev/sda1".to_string(),
                mount_point: "/".to_string(),
                file_system: "ext4".to_string(),
                total_bytes: 100,
                available_bytes: 10,
                is_removable: false,
            },
            SystemDiskDto {
                name: "/dev/sda2".to_string(),
                mount_point: "/home".to_string(),
                file_system: "ext4".to_string(),
                total_bytes: 100,
                available_bytes: 50,
                is_removable: false,
            },
        ];
        let free = disk_free_for(&disks, &PathBuf::from("/home/aliyss/models"));
        assert_eq!(free, Some(50));
        let free_root = disk_free_for(&disks, &PathBuf::from("/tmp"));
        assert_eq!(free_root, Some(10));
    }
}
