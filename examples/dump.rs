//! Native parity harness: decode real Konung files and print stable hashes so
//! the Rust port can be diffed against the original Python decoders.
//!
//! Usage: cargo run --release --example dump -- <game_dir> [profile] [variant] [location]

use std::collections::HashMap;
use std::path::PathBuf;

use openkonung::actors;
use openkonung::edges;
use openkonung::loc;
use openkonung::names;
use openkonung::population;
use openkonung::render;
use openkonung::res::Res;

fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn read(p: PathBuf) -> Option<Vec<u8>> {
    if p.exists() {
        Some(std::fs::read(&p).unwrap())
    } else {
        None
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let base = PathBuf::from(args.get(1).cloned().unwrap_or_else(|| "..".into()));
    let profile: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2);
    let variant_arg: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(-1);
    let loc_arg: u8 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(1);
    // Optional: dump one Entity's full authored state by source id (task §7).
    let entity_arg: Option<usize> = args.get(5).and_then(|s| s.parse().ok());

    let game = read(base.join("Game.res")).expect("Game.res");
    let graph = read(base.join("GRAPH.RES")).expect("GRAPH.RES");
    let objects = read(base.join("Objects.res")).expect("Objects.res");
    let exe_name = if base.join("CLIENT.EXE").exists() {
        "CLIENT.EXE"
    } else {
        "LAN_KONG.EXE"
    };
    let exe = read(base.join(exe_name)).expect("exe");

    let location_names = read(base.join("konung.res"))
        .map(|d| names::parse_location_names(&d))
        .unwrap_or_default();
    println!("location_names={}", location_names.len());
    for (i, n) in location_names.iter().take(20).enumerate() {
        println!("  name[{}]={}", i, n.escape_unicode());
    }

    let mut res = Res::new(game, graph, objects, exe);
    let variant = res
        .find_fallback_human(if variant_arg >= 0 {
            Some(variant_arg as usize)
        } else {
            None
        })
        .expect("fallback human");

    let p0: Vec<u8> = res.palettes[0].iter().flatten().copied().collect();
    let p36: Vec<u8> = res.palettes[36].iter().flatten().copied().collect();
    println!("profile={}", profile);
    println!("variant={}", variant);
    println!("palette0={:016x}", fnv(&p0));
    println!("palette36={:016x}", fnv(&p36));

    if let Some(t) = res.terrain_tile(0) {
        println!("tile0={:016x} {}x{}", fnv(&t.data), t.w, t.h);
    }
    if let Some(t) = res.terrain_tile(123) {
        println!("tile123={:016x} {}x{}", fnv(&t.data), t.w, t.h);
    }

    let (ps, gx, gy) = res.prop_sprite(30, 0, 0);
    println!(
        "prop30={:016x} {}x{} off={},{}",
        fnv(&ps.data),
        ps.w,
        ps.h,
        gx,
        gy
    );

    let (h, hx, hy, hc) = res.human_frame_state(variant, 0, 0, 0, None);
    println!(
        "human_a0_d0_f0={:016x} {}x{} off={},{} count={}",
        fnv(&h.data),
        h.w,
        h.h,
        hx,
        hy,
        hc
    );

    // locations
    let mut konungs: Vec<(u32, Vec<u8>)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&base) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if let Some(rest) = lower.strip_prefix("konung.") {
                if let Ok(id) = rest.parse::<u32>() {
                    if let Some(data) = read(e.path()) {
                        konungs.push((id, data));
                    }
                }
            }
        }
    }
    konungs.sort_by_key(|(id, _)| *id);
    let locs: Vec<loc::Location> = konungs
        .into_iter()
        .map(|(id, d)| loc::make_location(id, d).expect("Konung.N layout"))
        .collect();
    println!("loc_count={}", locs.len());

    let mut profiles: HashMap<u32, Vec<edges::Transition>> = HashMap::new();
    for p in 0..3u32 {
        if let Some(d) = read(base.join(format!("Game.{}", p))) {
            profiles.insert(p, edges::read_transitions(&d));
        }
    }
    let ev = edges::build_exact_edges(Some(profile), &profiles, &locs);
    let (pairs, one_way, self_edges) = edges::reciprocal_pairs(&ev);
    println!(
        "edges={} pairs={} one_way={} self={}",
        ev.len(),
        pairs.len(),
        one_way.len(),
        self_edges.len()
    );

    // Phase B: typed Group/Entity save-state tables + population enumeration.
    // Diagnostics only; no actor rendering and no coordinate conversion.
    let game_tables =
        read(base.join(format!("Game.{}", profile))).and_then(|gb| match population::parse_tables(
            &gb,
        ) {
            Ok(t) => Some(t),
            Err(e) => {
                println!("population parse error: {}", e);
                None
            }
        });

    if let (Some(tables), Some(id)) = (&game_tables, entity_arg) {
        match tables.entities.get(id) {
            Some(e) => println!("entity_state:\n{}", e.debug_block()),
            None => println!("entity_state: id {id} out of range"),
        }
    }

    if let Some(tables) = &game_tables {
        println!(
            "groups={} entities={}",
            tables.groups.len(),
            tables.entities.len()
        );
        let max_end = tables
            .groups
            .iter()
            .filter_map(|g| g.entity_range().ok())
            .map(|r| r.end)
            .max()
            .unwrap_or(0);
        println!("entity_range_max_end={}", max_end);
        assert!(
            max_end <= population::ENTITY_COUNT,
            "group entity range exceeds the {}-entry Entity table",
            population::ENTITY_COUNT
        );

        println!("population loc={}", loc_arg);
        for r in population::diagnose_location(tables, loc_arg) {
            let g = &r.group;
            match &r.range {
                Ok(range) => println!(
                    "  group {} loc {} class {} cap {} count18 {} first {} range {}..{} reserved {} active_class {}",
                    g.index,
                    g.loc_id,
                    g.class_1e,
                    g.capacity,
                    g.count_18,
                    g.first_entity,
                    range.start,
                    range.end,
                    r.reserved_group_zero,
                    r.valid_class
                ),
                Err(e) => println!(
                    "  group {} loc {} RANGE ERROR: {} (first {} count {})",
                    g.index, g.loc_id, e, g.first_entity, g.entity_count
                ),
            }
            for e in &r.entities {
                println!(
                    "    entity {} kind {} facing {} arch 0x{:02X} grp {} x {} y {} dlg {} name {} nick {} vis {} pal 0x{:X}",
                    e.index,
                    e.kind,
                    e.facing,
                    e.archetype,
                    e.group_id,
                    e.x,
                    e.y,
                    e.dialogue_id,
                    e.name_id,
                    e.nickname_id,
                    e.visual_index,
                    e.palette_offset
                );
            }
        }

        // Phase C1/C2: population selection, runtime placement and per-actor
        // visual resolution. Same contract as the app, without graphics.
        let collision: Vec<u16> = locs
            .iter()
            .find(|l| l.loc_id == loc_arg as u32)
            .map(|l| l.collision.clone())
            .unwrap_or_default();
        let pop = actors::build_population(tables, loc_arg, &collision);
        let mut sorted = pop.actors.clone();
        actors::sort_actors(&mut sorted);
        println!(
            "c2 loc={} matching_groups={} reserved_group_zero={} inactive_groups={} invalid_ranges={} enumerated_entities={} persisted_position_actors={} runtime_candidates={} runtime_placed={} runtime_failed={}",
            loc_arg,
            pop.matching_groups,
            pop.reserved_group_zero,
            pop.inactive_groups,
            pop.invalid_ranges,
            pop.enumerated_entities,
            pop.persisted_position_actors(),
            pop.runtime_candidates,
            pop.runtime_placed,
            pop.runtime_failed
        );
        let mut rejected = 0usize;
        for a in &sorted {
            let (rx, ry) = a.display_raw();
            let (lx, ly) =
                openkonung::coords::entity_to_local_px(rx.round() as i32, ry.round() as i32);
            let (lx, ly) = (lx as f64, ly as f64);
            let fam = match actors::visual_family(a.entity.archetype) {
                actors::VisualFamily::Human => "human",
                actors::VisualFamily::Object => "object",
            };
            if a.is_runtime() {
                let r = a.spawn_rect().unwrap_or_default();
                println!(
                    "  runtime entity={} group={} spawn_rect=({},{},{},{}) attempts={} raw_runtime_pos=({},{}) rendered_local_px=({:.1},{:.1}) validation=collision",
                    a.source_entity_id(),
                    a.source_group_id,
                    r.x1,
                    r.x2,
                    r.y1,
                    r.y2,
                    a.attempts().unwrap_or(0),
                    rx as i32,
                    ry as i32,
                    lx,
                    ly
                );
            }
            match res.actor_frame(
                &a.entity.equipment,
                a.entity.raw_0x12,
                openkonung::anim::battle_from_flags(a.entity.flags_b),
                a.entity.archetype,
                a.entity.visual_index,
                a.entity.palette_offset,
                a.entity.facing,
            ) {
                Ok((im, gx, gy)) => println!(
                    "  actor entity={} group={} runtime={} raw_pos=({},{}) local_px=({:.1},{:.1}) family={} archetype=0x{:02X} facing={} visual_index={} palette_offset=0x{:X} frame={}x{} off={},{}",
                    a.source_entity_id(),
                    a.source_group_id,
                    a.is_runtime(),
                    a.entity.x,
                    a.entity.y,
                    lx,
                    ly,
                    fam,
                    a.entity.archetype,
                    a.entity.facing,
                    a.entity.visual_index,
                    a.entity.palette_offset,
                    im.w,
                    im.h,
                    gx,
                    gy
                ),
                Err(reason) => {
                    rejected += 1;
                    println!(
                        "  actor entity={} group={} runtime={} raw_pos=({},{}) family={} archetype=0x{:02X} facing={} visual_index={} palette_offset=0x{:X} ERROR={}",
                        a.source_entity_id(),
                        a.source_group_id,
                        a.is_runtime(),
                        a.entity.x,
                        a.entity.y,
                        fam,
                        a.entity.archetype,
                        a.entity.facing,
                        a.entity.visual_index,
                        a.entity.palette_offset,
                        reason
                    );
                }
            }
        }
        for f in &pop.failed {
            println!(
                "  runtime entity={} group={} spawn_rect=({},{},{},{}) attempts={} placement_failed_after_100_attempts",
                f.source_entity_id,
                f.source_group_id,
                f.spawn_rect.x1,
                f.spawn_rect.x2,
                f.spawn_rect.y1,
                f.spawn_rect.y2,
                f.attempts
            );
        }
        println!("c2 rejected_visuals={}", rejected);
    }

    // R2 static-layer diagnostics (terrain both bytes, GRAPH anchors, props).
    if let Some(loc) = locs.iter().find(|l| l.loc_id == loc_arg as u32) {
        let cells = loc.terrain_cells();
        let (mut low_nz, mut high_nz) = (0usize, 0usize);
        for &rec in &cells {
            let (lo, hi) = openkonung::loc::terrain_layers(rec);
            if lo.is_some() {
                low_nz += 1;
            }
            if hi.is_some() {
                high_nz += 1;
            }
        }
        println!(
            "r2 terrain records={} low_nonzero={} high_nonzero={}",
            cells.len(),
            low_nz,
            high_nz
        );
        for (i, &rec) in cells.iter().enumerate().take(3) {
            let col = (i as i32) % openkonung::loc::LOCAL_TERRAIN_W;
            let row = (i as i32) / openkonung::loc::LOCAL_TERRAIN_W;
            let (lo, hi) = openkonung::loc::terrain_layers(rec);
            let (lx, ly) = openkonung::loc::terrain_to_local_px(col, row);
            println!(
                "  terrain record={} col={} row={} raw=0x{:04X} low={:?} high={:?} local=({},{})",
                i, col, row, rec, lo, hi, lx, ly
            );
        }

        println!("r2 graph count={}", loc.graph_placements.len());
        for (i, g) in loc.graph_placements.iter().enumerate().take(3) {
            let (lx, ly) = openkonung::loc::graph_anchor_to_local_px(g.x as i32, g.y as i32);
            println!(
                "  GRAPH record={} resource={} raw_anchor=({},{}) local_anchor=({},{})",
                i, g.tile, g.x, g.y, lx, ly
            );
        }

        println!("r2 props count={}", loc.props.len());
        for p in loc.props.iter().take(3) {
            let (lx, ly) = openkonung::loc::prop_to_local_px(p.grid_x, p.grid_y);
            println!(
                "  prop ri={} object_id={} grid_x={} grid_y={} local=({},{})",
                p.ri, p.object_id, p.grid_x, p.grid_y, lx, ly
            );
        }

        println!(
            "r2 surface={}x{}",
            openkonung::loc::LOCAL_PIXEL_W,
            openkonung::loc::LOCAL_PIXEL_H
        );
    }

    let layout = edges::spring_layout(&locs, &ev, &pairs);
    let mut ids: Vec<u32> = layout.keys().copied().collect();
    ids.sort_unstable();
    if let Some(&id) = ids.first() {
        let (x, y) = layout[&id];
        println!("spring0 id={} x={:.4} y={:.4}", id, x, y);
    }

    // Exercise the retained direct-render helpers (used again by the canonical
    // population phase). Base id 0 / palette 0 is a stable smoke input.
    let (im, gx, gy, c) = res.human_frame_from_base(&[0u16; 6], 0, false, 0, 0, 0, 0, 0);
    println!(
        "human_base vc=0 pal=0 {}x{} off={},{} count={} hash={:016x}",
        im.w,
        im.h,
        gx,
        gy,
        c,
        fnv(&im.data)
    );
    let (im, gx, gy, c) = res.monster_frame_from_object(30, 0, 0);
    println!(
        "monster_obj vc=30 pal=0 {}x{} off={},{} count={} hash={:016x}",
        im.w,
        im.h,
        gx,
        gy,
        c,
        fnv(&im.data)
    );

    // Render Konung.1 to validate the whole terrain+prop surface pipeline.
    if let Some(l1) = locs.iter().find(|l| l.loc_id == 1) {
        let surf = render::render_location_surface(&mut res, l1);
        println!("loc1={:016x} {}x{}", fnv(&surf.data), surf.w, surf.h);
    }
}
