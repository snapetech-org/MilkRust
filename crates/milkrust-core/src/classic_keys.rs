//! Classic serialized MilkDrop keys and their EEL runtime names. Raw source
//! keys remain in the document for round-trip preservation.
use std::collections::BTreeMap;
use crate::MilkRustValue;

pub const ALIASES: &[(&str, &str)] = &[
    ("frating", "rating"), ("fgammaadj", "gammaadj"), ("fdecay", "decay"),
    ("fvideoechozoom", "echo_zoom"), ("fvideoechoalpha", "echo_alpha"),
    ("nvideoechoorientation", "echo_orient"), ("nwavemode", "wave_mode"),
    ("badditivewaves", "additivewave"), ("bwavedots", "wave_dots"),
    ("bwavethick", "wave_thick"), ("bmodwavealphabyvolume", "modwavealphabyvolume"),
    ("bmaximizewavecolor", "maximizewavecolor"), ("btexwrap", "wrap"),
    ("bdarkencenter", "darken_center"), ("bredbluestereo", "red_blue"),
    ("bbrighten", "brighten"), ("bdarken", "darken"), ("bsolarize", "solarize"),
    ("binvert", "invert"), ("fwavealpha", "wave_a"), ("fwavescale", "wave_scale"),
    ("fwavesmoothing", "wave_smoothing"), ("fwaveparam", "wave_mystery"),
    ("fmodwavealphastart", "modwavealphastart"), ("fmodwavealphaend", "modwavealphaend"),
    ("fwarpanimspeed", "warpanimspeed"), ("fwarpscale", "warpscale"),
    ("fzoomexponent", "zoomexp"), ("fshader", "shader"),
    ("nmotionvectorsx", "mv_x"), ("nmotionvectorsy", "mv_y"),
    ("bspectrum", "spectrum"), ("busedots", "usedots"),
    ("bdrawthick", "thick"), ("badditive", "additive"),
];

pub fn runtime_name(key: &str) -> &str {
    ALIASES.iter().find(|(source, _)| *source == key).map_or(key, |(_, target)| *target)
}

pub fn value<'a>(values: &'a BTreeMap<String, MilkRustValue>, key: &str) -> Option<&'a MilkRustValue> {
    values.get(key).or_else(|| ALIASES.iter().find_map(|(source, target)| {
        if *target == key { values.get(*source) } else { None }
    }))
}

pub fn scope(values: &BTreeMap<String, MilkRustValue>) -> BTreeMap<String, MilkRustValue> {
    let mut result = values.clone();
    for (source, target) in ALIASES {
        if let Some(value) = values.get(*source) {
            result.entry((*target).to_string()).or_insert_with(|| value.clone());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_fields_seed_equations_without_losing_source_spelling() {
        let parsed = crate::parse_milkrust_preset_set("[preset00]\nfDecay=0.97\nnWaveMode=4\nfWaveAlpha=0.6\n", false);
        let values = &parsed.presets[0].base_values;
        let runtime = scope(values);
        assert_eq!(runtime.get("decay"), Some(&MilkRustValue::Number(0.97)));
        assert_eq!(runtime.get("wave_mode"), Some(&MilkRustValue::Number(4.0)));
        assert_eq!(value(values, "wave_a"), Some(&MilkRustValue::Number(0.6)));
        assert!(values.contains_key("fdecay"));
    }
}
