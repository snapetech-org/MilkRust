#!/usr/bin/env python3
"""Convert bounded Butterchurn 2.4.7 equations and GLSL without executing JS.

Every discovered JSON receives an accounting row. Unsupported code/shaders are
retained in the report, not silently accepted or counted as rendering successes.
Run conversion/packaging only on compatible private ephemeral JIT runners.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import re
import sys
from pathlib import Path
from urllib.parse import quote

sys.path.insert(0, str(Path(__file__).resolve().parent))
from butterchurn_equations import Equations, Unsupported, require
from butterchurn_glsl import convert_shader, ShaderConversionError

SOURCE = Path("content/third-party/butterchurn-presets-2.4.7")
SOURCE_URL = "https://github.com/jberg/butterchurn-presets"
MAX_SOURCE = 2 * 1024 * 1024
MAX_EQUATIONS = 65536
# Pinned reference defaults and MIT notice are shipped beside this tool.
DEFAULTS = json.loads((Path(__file__).parent / "data/butterchurn-defaults.json").read_text())
# Only field renaming; output remains ordinary portable MilkDrop text.
CLASSIC_KEYS = {
    "rating": "fRating", "gammaadj": "fGammaAdj", "decay": "fDecay",
    "echo_zoom": "fVideoEchoZoom", "echo_alpha": "fVideoEchoAlpha", "echo_orient": "nVideoEchoOrientation",
    "wave_mode": "nWaveMode", "additivewave": "bAdditiveWaves", "wave_dots": "bWaveDots",
    "wave_thick": "bWaveThick", "modwavealphabyvolume": "bModWaveAlphaByVolume",
    "maximizewavecolor": "bMaximizeWaveColor", "wrap": "bTexWrap", "darken_center": "bDarkenCenter",
    "red_blue": "bRedBlueStereo", "brighten": "bBrighten", "darken": "bDarken",
    "solarize": "bSolarize", "invert": "bInvert", "wave_a": "fWaveAlpha",
    "wave_scale": "fWaveScale", "wave_smoothing": "fWaveSmoothing", "wave_mystery": "fWaveParam",
    "modwavealphastart": "fModWaveAlphaStart", "modwavealphaend": "fModWaveAlphaEnd",
    "warpanimspeed": "fWarpAnimSpeed", "warpscale": "fWarpScale", "zoomexp": "fZoomExponent",
    "shader": "fShader", "mv_x": "nMotionVectorsX", "mv_y": "nMotionVectorsY",
}

def base_lines(values, prefix="", classic=False):
    require(isinstance(values, dict) and len(values) <= 256, "invalid base-value map")
    result = []
    for name, value in sorted(values.items()):
        require(re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]{0,127}", name), "invalid base-value name")
        require(type(value) in (int, float) and math.isfinite(value), "non-numeric base value")
        result.append(f"{prefix}{CLASSIC_KEYS.get(name, name) if classic else name}={value:.17g}")
    return result


def equation_lines(source, prefix):
    return [f"{prefix}{index}={line}" for index, line in enumerate(Equations(source).statements(), 1)]


def convert(data, title):
    require(isinstance(data, dict), "JSON root must be an object")
    allowed = {"baseVals", "shapes", "waves", "init_eqs_str", "frame_eqs_str", "pixel_eqs_str", "warp", "comp"}
    require(not (set(data) - allowed), "unknown top-level fields require review")
    shaders = {}
    for stage in ("warp", "comp"):
        value = data.get(stage, "")
        require(isinstance(value, str), "shader must be a string")
        if value.strip():
            try:
                shaders[stage] = convert_shader(value)
            except ShaderConversionError as error:
                raise Unsupported(f"{stage}: {error}") from error
    require("\n" not in title and "\r" not in title, "invalid preset title")
    lines = ["MILKDROP_PRESET_VERSION=201", "PSVERSION=3" if shaders else "PSVERSION=0",
             "// MILKRUST_REQUIRED_PROFILE=butterchurn-numeric-v1",
             "[preset00]", "name=" + title]
    require(isinstance(data.get("baseVals", {}), dict), "invalid preset base values")
    lines += base_lines({**DEFAULTS["preset"], **data.get("baseVals", {})}, classic=True)
    for key, prefix in (("init_eqs_str", "per_frame_init_"), ("frame_eqs_str", "per_frame_"), ("pixel_eqs_str", "per_pixel_")):
        lines += equation_lines(data.get(key, ""), prefix)
    for family, classic in (("shapes", "shapecode"), ("waves", "wavecode")):
        objects = data.get(family, [])
        require(isinstance(objects, list) and len(objects) <= 64, "indexed object limit")
        for index, entry in enumerate(objects):
            require(isinstance(entry, dict) and not (set(entry) - {"baseVals", "init_eqs_str", "frame_eqs_str", "point_eqs_str"}), "unknown indexed-object fields")
            require(isinstance(entry.get("baseVals", {}), dict), "invalid object base values")
            values = {**DEFAULTS[family], **entry.get("baseVals", {})}
            if family == "waves":
                keys = {"spectrum": "bSpectrum", "usedots": "bUseDots", "thick": "bDrawThick", "additive": "bAdditive"}
                values = {keys.get(k, k): v for k, v in values.items()}
            lines += base_lines(values, f"{classic}_{index}_")
            for key, phase in (("init_eqs_str", "init"), ("frame_eqs_str", "per_frame"), ("point_eqs_str", "per_point")):
                require(family == "waves" or key != "point_eqs_str" or not entry.get(key), "per-point shape code is unsupported")
                prefix = f"{'shape' if family == 'shapes' else 'wave'}_{index}_{phase}"
                lines += equation_lines(entry.get(key, ""), prefix)
    for stage, source in shaders.items():
        lines += [f"{stage}_{index}={line}" for index, line in enumerate(source.splitlines(), 1)]
    result = "\n".join(lines) + "\n"
    require(len(result.encode()) <= MAX_SOURCE, "converted preset exceeds limit")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-revision", required=True, help="Full MilkRust commit containing the vendored source")
    parser.add_argument("--pack-size", type=int, default=200)
    args = parser.parse_args()
    require(re.fullmatch(r"[a-f0-9]{40}", args.source_revision), "source revision must be immutable")
    require(1 <= args.pack_size <= 500, "invalid pack size")
    require(not args.output.exists(), "output must be a new directory")
    license_data = (args.source / "LICENSE").read_bytes()
    package = json.loads((args.source / "package.json").read_text())
    require(package.get("version") == "2.4.7" and package.get("license") == "MIT"
            and b"MIT License" in license_data, "unrecognized source package license/version")
    args.output.mkdir(parents=True)
    rows, accepted, corpus_hash = [], [], hashlib.sha256()
    for source in sorted((args.source / "presets/converted").glob("*.json")):
        row = {"file": source.name, "status": "rejected", "rendered": False, "qualified": False}
        try:
            require(not source.is_symlink() and source.stat().st_size <= MAX_SOURCE, "linked or oversized source")
            data = source.read_bytes()
            digest = hashlib.sha256(data).hexdigest()
            row["sha256"] = digest
            corpus_hash.update(source.name.encode() + b"\0" + digest.encode() + b"\n")
            decoded = json.loads(data)
            text = convert(decoded, source.stem)
            row["shaderStages"] = [stage for stage in ("warp", "comp") if decoded.get(stage, "").strip()]
            row["convertedSha256"] = hashlib.sha256(text.encode()).hexdigest()
            row["requiredProfile"] = "butterchurn-numeric-v1"
            # Original filename/title remain in attribution. Portable filename
            # derives from content identity, preventing cross-platform collisions.
            row["status"] = "converted-unqualified"
            row["destination"] = digest + ".milk"
            accepted.append((source, row, text))
        except (OSError, UnicodeError, ValueError, TypeError, AttributeError) as error:
            row["reason"] = str(error)
        rows.append(row)
    require(rows, "source inventory is empty")
    for offset in range(0, len(accepted), args.pack_size):
        part = offset // args.pack_size + 1
        pack_id = f"butterchurn-converted-{part:02}"
        root = args.output / pack_id
        (root / "presets").mkdir(parents=True)
        (root / "licenses").mkdir()
        (root / "licenses/MIT.txt").write_bytes(license_data)
        presets, assets = [], []
        for source, row, text in accepted[offset:offset+args.pack_size]:
            path = "presets/" + row["destination"]
            (root / path).write_text(text, encoding="utf-8")
            original = source.stem
            # Filename credits are retained, not independently asserted as legal authorship.
            credits = original.split(" - ", 1)[0] if " - " in original else "Butterchurn preset contributors"
            source_url = f"https://github.com/snapetech-org/MilkRust/blob/{args.source_revision}/{SOURCE.as_posix()}/presets/converted/{quote(source.name)}"
            presets.append({"id": row["sha256"], "file": path, "title": original, "author": credits,
                            "sourceFormat": "milk", "requiredEngine": "milkrust",
                            "requiredProfile": "butterchurn-numeric-v1",
                            "tags": ["butterchurn", "converted", "shader-hlsl" if row["shaderStages"] else "shaderless"]})
            assets.append({"path": path, "kind": "preset", "license": "NOASSERTION", "rightsStatus": "review-required",
                           "author": credits, "sourceUrl": source_url, "sourceSha256": row["sha256"],
                           "originalFilename": source.name,
                           "convertedSha256": row["convertedSha256"],
                           "transformation": "bounded numeric equations to EEL and generated GLSL to typed legacy HLSL; no source execution"})
            row["packId"] = pack_id
        manifest = {"schemaVersion": 1, "id": pack_id, "version": "2.4.7.2", "name": f"Butterchurn Converted {part}",
                    "license": "MIT", "licenseFiles": ["licenses/MIT.txt"],
                    "distributionStatus": "rights-review-required", "rightsReviewStatus": "required",
                    "sourceUrls": [SOURCE_URL, "https://www.npmjs.com/package/butterchurn-presets/v/2.4.7"],
                    "contentNotice": "Converted from butterchurn-presets 2.4.7; the upstream package declares MIT and its notice is included. Per-preset rights-holder chains have not been independently confirmed, so every asset is NOASSERTION and this archive is not eligible for public catalog promotion. Original filename credits are retained. Bounded declarative equation and shader conversion; MilkRust numeric profile required. Rendering fidelity has not been qualified. Third-party names may contain mature language. No downloaded JavaScript is executed.",
                    "presetRoot": "presets", "presets": presets, "assets": assets,
                    "compatibilityStatus": "converted-unqualified", "sourceRevision": args.source_revision}
        (root / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    report = {"schemaVersion": 1, "sourceRevision": args.source_revision, "sourceCorpusSha256": corpus_hash.hexdigest(),
              "discovered": len(rows), "converted": len(accepted), "rejected": len(rows)-len(accepted),
              "rendered": 0, "qualified": 0, "rows": rows}
    (args.output / "conversion-report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({key: report[key] for key in ("discovered", "converted", "rejected", "rendered", "qualified")}))


if __name__ == "__main__":
    try: main()
    except (OSError, ValueError, KeyError) as error: raise SystemExit(f"conversion failed: {error}")
