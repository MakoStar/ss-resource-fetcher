use crate::config::ClientRegistryConfig;

#[derive(Debug, Clone, Copy)]
pub struct PatchRef<'a> {
    /// 补丁文件名
    pub name: &'a str,
    /// 补丁版本
    pub version: u64,
    /// 补丁序号
    pub sequence: u64,
}

impl<'a> PatchRef<'a> {
    pub fn new(registry: &ClientRegistryConfig, name: &'a str, version: u64) -> Self {
        Self {
            name,
            version,
            sequence: registry.patch_sequence(name),
        }
    }
}

pub fn tier_label(tier: u64) -> String {
    if tier == 0 {
        return String::from("tier 0 (base)");
    }

    format!("tier {tier}")
}

pub fn apply_order<'a>(
    base_version: Option<u64>,
    patches: &[PatchRef<'a>],
    is_new_file: impl Fn(&str) -> bool,
) -> Vec<&'a str> {
    let mut ordered: Vec<&PatchRef<'a>> = patches.iter().collect();
    ordered.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then(left.version.cmp(&right.version))
            .then(left.name.cmp(right.name))
    });

    let mut kept: Vec<&'a str> = Vec::new();
    let mut current = base_version;
    let mut applied = false;

    for patch in ordered {
        let outdated = match current {
            Some(version) if !applied => patch.version <= version,
            Some(version) => patch.version < version,
            None => false,
        };

        if outdated {
            continue;
        }

        if is_new_file(patch.name) {
            kept.clear();
        }

        kept.push(patch.name);
        current = Some(patch.version);
        applied = true;
    }

    kept
}
