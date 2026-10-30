"""Bounded immutable texture-pack dependency schema shared by release tools."""
import re

_COMPONENT = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$")
_DIGEST = re.compile(r"^[a-f0-9]{64}$")


def validate_pack_dependencies(manifest):
    kind = manifest.get("kind", "presets")
    if kind not in ("presets", "textures"):
        raise ValueError("unknown pack kind")
    presets = manifest.get("presets", [])
    if not isinstance(presets, list) or len(presets) > 2500:
        raise ValueError("invalid or oversized preset inventory")
    for preset in presets:
        if not isinstance(preset, dict):
            raise ValueError("invalid preset entry")
        if "requiredEngine" in preset and preset["requiredEngine"] != "milkrust":
            raise ValueError("unknown required preset engine")
        if "requiredProfile" in preset and (preset["requiredProfile"] != "butterchurn-numeric-v1"
                                            or preset.get("requiredEngine") != "milkrust"):
            raise ValueError("unknown or unbound preset conversion profile")
    dependencies = manifest.get("dependencies", [])
    if not isinstance(dependencies, list) or len(dependencies) > 16:
        raise ValueError("at most 16 texture dependencies are allowed")
    seen = set()
    for dependency in dependencies:
        if not isinstance(dependency, dict):
            raise ValueError("invalid texture dependency")
        for field in ("id", "version"):
            value = dependency.get(field)
            if (not isinstance(value, str) or not _COMPONENT.fullmatch(value)
                    or value.endswith((".", ".previous", ".partial"))
                    or re.match(r"^(con|prn|aux|nul|com[0-9]|lpt[0-9])($|\.)", value, re.I)):
                raise ValueError("invalid texture dependency identity")
        digest = dependency.get("integritySha256")
        if not isinstance(digest, str) or not _DIGEST.fullmatch(digest):
            raise ValueError("texture dependency must pin integrity.json SHA-256")
        identity = (dependency["id"], dependency["version"])
        if identity == (manifest.get("id"), manifest.get("version")):
            raise ValueError("pack cannot depend on itself")
        if identity in seen:
            raise ValueError("duplicate texture dependency")
        seen.add(identity)
    if kind == "textures":
        if dependencies or manifest.get("presets") != []:
            raise ValueError("texture packs must be self-contained and have no presets")
        textures = manifest.get("textures")
        if not isinstance(textures, list) or not 0 < len(textures) <= 2500:
            raise ValueError("texture packs must declare a bounded texture inventory")
    return kind
