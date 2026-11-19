---
id: 04-ogimage
type: image
agent: agy (gen-image.ps1)
acceptance: [I1]
---

# Task 04: Social preview image

Prompt for gen-image:
"Wide 1200x630 social preview banner. A dreamy pastel micro-voxel diorama: a small floating island
with a Japanese temple and blooming pink cherry blossom trees, tiny waterfalls falling off its edge,
soft clouds below, warm golden-hour light, isometric tilt-shift look. Island placed in the right half;
left 40% is calm soft sky gradient (empty, for a title). No text, no logos."

Then (Claude): resize/crop to exactly 1200×630 PNG → `web/og-image.png`; add to `web/index.html`:
`og:title`, `og:description`, `og:image`, `twitter:card=summary_large_image`.
