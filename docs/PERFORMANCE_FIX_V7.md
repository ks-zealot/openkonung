# Performance fix v7

The 2.5 FPS at whole-country zoom was not caused by ~700 cached sprite frames.
Those HUD counters are cache sizes, not per-frame draw counts.

The actual bug was the zoom-out render path:

- at ~0.057x on a 1600x800 window it created a world-resolution intermediate surface tens of thousands of pixels wide;
- it redrew terrain/props/entities into that giant buffer;
- then `pygame.transform.smoothscale` reduced the entire buffer to the physical window every frame.

That is hundreds of millions of source pixels per frame, in software.

## New path

For zoom <= 0.30:

1. Build a static world mip atlas lazily at 1/16, 1/8 or 1/4 scale.
2. Bake terrain + static props into that atlas once.
3. Cache the final screen-sized static view while camera/zoom are unchanged.
4. Each frame draw only living animated entities on top.
5. Runtime FPS is uncapped (`Clock.tick(0)`) instead of artificially capped at 60.

The normal close-up renderer is unchanged except for clipping partial terrain chunks so their padding cannot spill into adjacent stitched maps.
