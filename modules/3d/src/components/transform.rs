use crayon::ecs::prelude::*;
use crayon::math;
use crayon::math::Transform as _Transform;
use crayon::math::{Matrix, One, Rotation};

use components::node::Node;
use errors::*;

/// `Transform` is used to store and manipulate the postiion, rotation and scale
/// of the object. We use a left handed, y-up world coordinate system.
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    decomposed: math::Decomposed<math::Vector3<f32>, math::Quaternion<f32>>,
}

/// Declare `Transform` as component with compact vec storage.
impl Component for Transform {
    type Arena = VecArena<Transform>;
}

impl Default for Transform {
    fn default() -> Self {
        Transform {
            decomposed: math::Decomposed::one(),
        }
    }
}

impl Transform {
    /// Get the scale component in local space.
    #[inline]
    pub fn scale(&self) -> f32 {
        self.decomposed.scale
    }

    /// Set the scale component in local space.
    #[inline]
    pub fn set_scale(&mut self, scale: f32) {
        self.decomposed.scale = scale;
    }

    #[inline]
    pub fn position(&self) -> math::Vector3<f32> {
        self.decomposed.disp
    }

    #[inline]
    pub fn set_position<T>(&mut self, position: T)
    where
        T: Into<math::Vector3<f32>>,
    {
        self.decomposed.disp = position.into();
    }

    #[inline]
    pub fn translate<T>(&mut self, disp: T)
    where
        T: Into<math::Vector3<f32>>,
    {
        self.decomposed.disp += disp.into();
    }

    #[inline]
    pub fn rotation(&self) -> math::Quaternion<f32> {
        self.decomposed.rot
    }

    #[inline]
    pub fn set_rotation<T>(&mut self, rotation: T)
    where
        T: Into<math::Quaternion<f32>>,
    {
        self.decomposed.rot = rotation.into();
    }

    #[inline]
    pub fn rotate<T>(&mut self, rotate: T)
    where
        T: Into<math::Quaternion<f32>>,
    {
        self.decomposed.rot = rotate.into() * self.decomposed.rot;
    }
}

impl Transform {
    /// Get the transform matrix from local space to world space.
    pub fn world_matrix<T1, T2>(tree: &T1, arena: &T2, handle: Entity) -> Result<math::Matrix4<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        let decomposed = Transform::world_decomposed(tree, arena, handle)?;
        Ok(math::Matrix4::from(decomposed))
    }

    /// Get the view matrix from world space to view space.
    pub fn world_view_matrix<T1, T2>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
    ) -> Result<math::Matrix4<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        let decomposed = Transform::world_decomposed(tree, arena, handle)?;
        let it = math::Matrix4::from_translation(-decomposed.disp);
        let ir = math::Matrix4::from(decomposed.rot).transpose();
        // M = ( T * R ) ^ -1
        Ok(ir * it)
    }

    /// Get the transform matrix from world space to local space.
    pub fn inverse_world_matrix<T1, T2>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
    ) -> Result<math::Matrix4<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        let decomposed = Transform::world_decomposed(tree, arena, handle)?;
        if let Some(inverse) = decomposed.inverse_transform() {
            Ok(math::Matrix4::from(inverse))
        } else {
            Err(Error::CanNotInverseTransform)
        }
    }

    /// Set position of `Transform` in world space.
    pub fn set_world_position<T1, T2, T3>(
        tree: &T1,
        arena: &mut T2,
        handle: Entity,
        disp: T3,
    ) -> Result<()>
    where
        T1: Arena<Node>,
        T2: ArenaMut<Transform>,
        T3: Into<math::Vector3<f32>>,
    {
        if arena.get(handle).is_none() {
            return Err(Error::NonTransformFound);
        }

        unsafe {
            Self::set_world_position_unchecked(tree, arena, handle, disp);
            Ok(())
        }
    }

    /// Set position of `Transform` in world space without doing bounds checking.
    pub unsafe fn set_world_position_unchecked<T1, T2, T3>(
        tree: &T1,
        arena: &mut T2,
        handle: Entity,
        disp: T3,
    ) where
        T1: Arena<Node>,
        T2: ArenaMut<Transform>,
        T3: Into<math::Vector3<f32>>,
    {
        let disp = disp.into();
        if tree.get(handle).is_none() {
            arena.get_unchecked_mut(handle).set_position(disp);
        } else {
            let mut ancestors_disp = math::Vector3::new(0.0, 0.0, 0.0);
            for v in Node::ancestors(tree, handle) {
                if let Some(transform) = arena.get(v) {
                    ancestors_disp += transform.position();
                }
            }

            arena
                .get_unchecked_mut(handle)
                .set_position(disp - ancestors_disp);
        }
    }

    /// Get position of `Transform` in world space.
    pub fn world_position<T1, T2>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
    ) -> Result<math::Vector3<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        if arena.get(handle).is_none() {
            Err(Error::NonTransformFound)
        } else {
            unsafe { Ok(Self::world_position_unchecked(tree, arena, handle)) }
        }
    }

    /// Get position of `Transform` in world space without doing bounds checking.
    pub unsafe fn world_position_unchecked<T1, T2>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
    ) -> math::Vector3<f32>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        let transform = arena.get_unchecked(handle);
        let mut disp = transform.position();
        for v in Node::ancestors(tree, handle) {
            if let Some(ancestor) = arena.get(v) {
                disp += ancestor.position();
            }
        }

        disp
    }

    /// Set uniform scale of `Transform` in world space.
    pub fn set_world_scale<T1, T2>(
        tree: &T1,
        arena: &mut T2,
        handle: Entity,
        scale: f32,
    ) -> Result<()>
    where
        T1: Arena<Node>,
        T2: ArenaMut<Transform>,
    {
        if arena.get(handle).is_none() {
            return Err(Error::NonTransformFound);
        }

        unsafe {
            Self::set_world_scale_unchecked(tree, arena, handle, scale);
            Ok(())
        }
    }

    /// Set uniform scale of `Transform` in world space withoud doing bounds checking.
    pub unsafe fn set_world_scale_unchecked<T1, T2>(
        tree: &T1,
        arena: &mut T2,
        handle: Entity,
        scale: f32,
    ) where
        T1: Arena<Node>,
        T2: ArenaMut<Transform>,
    {
        if tree.get(handle).is_none() {
            arena.get_unchecked_mut(handle).set_scale(scale);
        } else {
            let mut ancestors_scale = 1.0;
            for v in Node::ancestors(tree, handle) {
                if let Some(transform) = arena.get(v) {
                    ancestors_scale *= transform.scale();
                }
            }

            if ancestors_scale < ::std::f32::EPSILON {
                arena.get_unchecked_mut(handle).set_scale(scale);
            } else {
                arena
                    .get_unchecked_mut(handle)
                    .set_scale(scale / ancestors_scale);
            }
        }
    }

    /// Get the scale of `Transform` in world space.
    pub fn world_scale<T1, T2>(tree: &T1, arena: &T2, handle: Entity) -> Result<f32>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        if arena.get(handle).is_none() {
            Err(Error::NonTransformFound)
        } else {
            unsafe { Ok(Self::world_scale_unchecked(tree, arena, handle)) }
        }
    }

    /// Get the scale of `Transform` in world space without doing bounds checking.
    pub unsafe fn world_scale_unchecked<T1, T2>(tree: &T1, arena: &T2, handle: Entity) -> f32
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        let transform = arena.get_unchecked(handle);
        let mut scale = transform.scale();
        for v in Node::ancestors(tree, handle) {
            if let Some(ancestor) = arena.get(v) {
                scale *= ancestor.scale();
            }
        }
        scale
    }

    /// Set rotation of `Transform` in world space.
    pub fn set_world_rotation<T1, T2, T3>(
        tree: &T1,
        arena: &mut T2,
        handle: Entity,
        rotation: T3,
    ) -> Result<()>
    where
        T1: Arena<Node>,
        T2: ArenaMut<Transform>,
        T3: Into<math::Quaternion<f32>>,
    {
        if arena.get(handle).is_none() {
            return Err(Error::NonTransformFound);
        }

        unsafe {
            Self::set_world_rotation_unchecked(tree, arena, handle, rotation);
            Ok(())
        }
    }

    /// Set rotation of `Transform` in world space without doing bounds checking.
    pub unsafe fn set_world_rotation_unchecked<T1, T2, T3>(
        tree: &T1,
        arena: &mut T2,
        handle: Entity,
        rotation: T3,
    ) where
        T1: Arena<Node>,
        T2: ArenaMut<Transform>,
        T3: Into<math::Quaternion<f32>>,
    {
        if tree.get(handle).is_none() {
            arena.get_unchecked_mut(handle).set_rotation(rotation);
        } else {
            let mut ancestors_rotation = math::Quaternion::one();
            for v in Node::ancestors(tree, handle) {
                if let Some(transform) = arena.get(v) {
                    ancestors_rotation = ancestors_rotation * transform.rotation();
                }
            }

            arena
                .get_unchecked_mut(handle)
                .set_rotation(rotation.into() * ancestors_rotation.invert());
        }
    }

    /// Get rotation of `Transform` in world space.
    pub fn world_rotation<T1, T2>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
    ) -> Result<math::Quaternion<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        if arena.get(handle).is_none() {
            Err(Error::NonTransformFound)
        } else {
            unsafe { Ok(Self::world_rotation_unchecked(tree, arena, handle)) }
        }
    }

    /// Get rotation of `Transform` in world space without doing bounds checking.
    pub unsafe fn world_rotation_unchecked<T1, T2>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
    ) -> math::Quaternion<f32>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        let transform = arena.get_unchecked(handle);
        let mut rotation = transform.rotation();
        for v in Node::ancestors(tree, handle) {
            if let Some(ancestor) = arena.get(v) {
                rotation = rotation * ancestor.rotation();
            }
        }

        rotation
    }

    #[allow(dead_code)]
    pub(crate) fn set_world_decomposed<T1, T2>(
        tree: &T1,
        arena: &mut T2,
        handle: Entity,
        decomposed: math::Decomposed<math::Vector3<f32>, math::Quaternion<f32>>,
    ) -> Result<()>
    where
        T1: Arena<Node>,
        T2: ArenaMut<Transform>,
    {
        let relative = Transform::world_decomposed(tree, arena, handle)?;

        if let Some(inverse) = relative.inverse_transform() {
            unsafe {
                arena.get_unchecked_mut(handle).decomposed = inverse.concat(&decomposed);
            }
            Ok(())
        } else {
            Err(Error::CanNotInverseTransform)
        }
    }

    pub(crate) fn world_decomposed<T1, T2>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
    ) -> Result<math::Decomposed<math::Vector3<f32>, math::Quaternion<f32>>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        if arena.get(handle).is_none() {
            Err(Error::NonTransformFound)
        } else {
            unsafe { Ok(Self::world_decomposed_unchecked(tree, arena, handle)) }
        }
    }

    pub(crate) unsafe fn world_decomposed_unchecked<T1, T2>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
    ) -> math::Decomposed<math::Vector3<f32>, math::Quaternion<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        let transform = arena.get_unchecked(handle);
        let mut decomposed = transform.decomposed;
        for v in Node::ancestors(tree, handle) {
            if let Some(ancestor) = arena.get(v) {
                decomposed = ancestor.decomposed.concat(&decomposed);
            }
        }
        decomposed
    }
}

impl Transform {
    /// Transforms position from local space to world space.
    pub fn transform_point<T1, T2, T3>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
        v: T3,
    ) -> Result<math::Vector3<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
        T3: Into<math::Vector3<f32>>,
    {
        let decomposed = Transform::world_decomposed(tree, arena, handle)?;
        // M = T * R * S
        Ok(decomposed.rot * (v.into() * decomposed.scale) + decomposed.disp)
    }

    /// Transforms vector from local space to world space.
    ///
    /// This operation is not affected by position of the transform, but is is affected by scale.
    /// The returned vector may have a different length than vector.
    pub fn transform_vector<T1, T2, T3>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
        v: T3,
    ) -> Result<math::Vector3<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
        T3: Into<math::Vector3<f32>>,
    {
        let decomposed = Transform::world_decomposed(tree, arena, handle)?;
        Ok(decomposed.transform_vector(v.into()))
    }

    /// Transforms direction from local space to world space.
    ///
    /// This operation is not affected by scale or position of the transform. The returned
    /// vector has the same length as direction.
    pub fn transform_direction<T1, T2, T3>(
        tree: &T1,
        arena: &T2,
        handle: Entity,
        v: T3,
    ) -> Result<math::Vector3<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
        T3: Into<math::Vector3<f32>>,
    {
        let rotation = Transform::world_rotation(tree, arena, handle)?;
        Ok(rotation * v.into())
    }

    /// Return the up direction in world space, which is looking down the positive y-axis.
    pub fn up<T1, T2>(tree: &T1, arena: &T2, handle: Entity) -> Result<math::Vector3<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        Transform::transform_direction(tree, arena, handle, math::Vector3::new(0.0, 1.0, 0.0))
    }

    /// Return the forward direction in world space, which is looking down the positive z-axis.
    pub fn forward<T1, T2>(tree: &T1, arena: &T2, handle: Entity) -> Result<math::Vector3<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        Transform::transform_direction(tree, arena, handle, math::Vector3::new(0.0, 0.0, 1.0))
    }

    /// Return the right direction in world space, which is looking down the positive x-axis.
    pub fn right<T1, T2>(tree: &T1, arena: &T2, handle: Entity) -> Result<math::Vector3<f32>>
    where
        T1: Arena<Node>,
        T2: Arena<Transform>,
    {
        Transform::transform_direction(tree, arena, handle, math::Vector3::new(1.0, 0.0, 0.0))
    }
}
