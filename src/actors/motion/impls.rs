//! RuntimeActor accessors.

use super::super::*;

impl RuntimeActor {
    /// Source Entity id (`Entity` table index).
    pub fn source_entity_id(&self) -> usize {
        self.entity.index
    }

    /// True when the position was assigned at runtime (the authored cell was
    /// `(0,0)`), false when the saved cell is authoritative.
    pub fn is_runtime(&self) -> bool {
        matches!(self.placement, ActorPlacement::Runtime { .. })
    }

    /// 1-based placement attempt that produced a runtime cell, or `None` for a
    /// persisted actor.
    pub fn attempts(&self) -> Option<u8> {
        match self.placement {
            ActorPlacement::Persisted => None,
            ActorPlacement::Runtime { attempts, .. } => Some(attempts),
        }
    }

    /// Spawn rectangle A used for a runtime placement, if this actor is runtime.
    pub fn spawn_rect(&self) -> Option<Rect> {
        match self.placement {
            ActorPlacement::Persisted => None,
            ActorPlacement::Runtime { spawn_rect, .. } => Some(spawn_rect),
        }
    }

    /// Entity/transition-domain coordinates to project for drawing. Runtime
    /// placement overrides the authored `(0,0)` position; the entity is unchanged.
    pub fn display_raw(&self) -> (f64, f64) {
        match self.placement {
            ActorPlacement::Persisted => (self.entity.x as f64, self.entity.y as f64),
            ActorPlacement::Runtime { x, y, .. } => (x as f64, y as f64),
        }
    }
}
