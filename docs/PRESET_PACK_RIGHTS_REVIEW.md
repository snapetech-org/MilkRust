# Preset pack rights review

This is a source review for pack planning, not a legal determination. A public
repository, a permissive application license, long-standing community use, or a
disclaimer does not by itself establish that every preset author authorized
MilkRust to redistribute and transform their work.

## Reviewed sources

| Source | Upstream evidence | Current disposition |
| --- | --- | --- |
| `butterchurn-presets@2.4.7` | [The upstream repository describes the presets as converted from MilkDrop and declares MIT](https://github.com/jberg/butterchurn-presets). The vendored package includes that notice. | Candidate conversion is useful for compatibility review. Every asset stays `NOASSERTION` until the grant and attribution chain for the underlying preset is approved. |
| projectM Cream of the Crop | [The upstream license says most presets had no specific license, notes authors theoretically hold copyright, and then makes a public-domain assumption based on free release and reuse](https://github.com/projectM-visualizer/presets-cream-of-the-crop/blob/master/LICENSE.md). | Link/import only. The public-domain assumption is not an author grant. |
| projectM En D collection | [The source describes about 50 presets created or remixed by En D](https://github.com/projectM-visualizer/presets-en-d), but its [license file repeats the no-specific-license/public-domain-assumption language](https://github.com/projectM-visualizer/presets-en-d/blob/master/LICENSE.md). | Link/import only until the relevant authors grant redistribution rights. |
| Original MilkDrop pack and other historical archives | No reviewed per-asset license or rights-holder evidence is present in MilkRust. | Link/import only. |

The current `content/downloads/catalog.json` therefore keeps Cream of the Crop
and Original MilkDrop as `NOASSERTION` link-only entries. No larger community
archive has passed the rights gate. The MIT Butterchurn source is retained as
third-party material under its upstream package declaration, while generated
candidate assets are explicitly marked `NOASSERTION` and
`rights-review-required`.

## Promotion evidence required

Before a broader pack can become an in-app download, retain a reviewer-approved
rights record bound to the exact archive digest. The record must cover every
asset and any conversion or derivative work, identify the rights holder and
source, specify the redistribution and transformation license, link the actual
license text or permission grant, and preserve required attribution. Remove or
replace any preset whose chain cannot be established. Then complete the
separate renderer qualification matrix in
[`content/qualification/README.md`](../content/qualification/README.md).

The review-candidate packager intentionally does not turn missing evidence into
a redistribution grant. Candidate archives are for internal review only; the
public catalog validator and signed-catalog publisher reject
`distributionStatus: rights-review-required`. The signed-catalog path also
requires approved rights evidence bound to the exact archive digest through
`tools/catalog_qualification.py`.
