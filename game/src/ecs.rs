//! The game's `u32` entity ids against the engine's `Entity`-keyed queries.
//!
//! The game keeps entities as bare `u32` ids — a chassis, a wheel, a camera — and until the pin
//! moved past `eaacb22e` the engine's storage views took them as they were. A `Query` now asks for
//! an [`Entity`](gizmo::core::entity::Entity), generation included. [`ById::by_id`] and
//! [`ByIdMut::by_id_mut`] look the id up as the live entity of that index first, which is what the
//! old views did implicitly, so the call sites stay one call each.

use gizmo::core::query::{Query, QueryData, QueryFilter, ReadOnlyQuery, WorldQuery};

/// Read access by `u32` id.
pub trait ById<D: WorldQuery> {
    /// The row of the live entity with index `id`, if it has one in this query.
    fn by_id(&self, id: u32) -> Option<D::Item<'_>>;
}

/// Write access by `u32` id.
pub trait ByIdMut<D: WorldQuery> {
    /// The row of the live entity with index `id`, if it has one in this query, for writing.
    fn by_id_mut(&mut self, id: u32) -> Option<D::Item<'_>>;
}

impl<D: QueryData + ReadOnlyQuery, F: QueryFilter> ById<D> for Query<'_, D, F> {
    fn by_id(&self, id: u32) -> Option<D::Item<'_>> {
        self.get(self.entity(id)?)
    }
}

impl<D: QueryData, F: QueryFilter> ByIdMut<D> for Query<'_, D, F> {
    fn by_id_mut(&mut self, id: u32) -> Option<D::Item<'_>> {
        let e = self.entity(id)?;
        self.get_mut(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizmo::physics::components::Velocity;
    use gizmo::prelude::*;

    /// The id the game keeps finds the row again, through a fresh query each way, and a write
    /// through `by_id_mut` is what the next `by_id` reads.
    #[test]
    fn a_u32_id_finds_its_row() {
        let mut world = World::new();
        let _before = world.spawn();
        let e = world.spawn();
        world.add_component(e, Velocity::default());
        let id = e.id();
        let ids: Vec<u32> =
            world.query::<(gizmo::core::entity::Entity, &Velocity)>().iter().map(|(e, _)| e.id()).collect();
        assert_eq!(ids, vec![id]);
        assert!(world.query::<&Velocity>().by_id(id).is_some());
        if let Some(mut v) = world.query_mut::<&mut Velocity>().by_id_mut(id) {
            v.linear = Vec3::X;
        }
        assert_eq!(world.query::<&Velocity>().by_id(id).map(|v| v.linear), Some(Vec3::X));
    }
}
