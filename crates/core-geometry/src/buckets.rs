use crate::{FastMap, Scalar, Vec2, Vec3};

/// The cube `cell_mm` across that `at` falls in, of a grid with a corner at the origin.
pub fn cell_of_point(at: Vec3, cell_mm: Scalar) -> [i32; 3] {
    (at / cell_mm).floor().as_ivec3().to_array()
}

/// The 27 cells of the cube three cells across centred on `cell`, `cell` among them.
pub fn cells_around([x, y, z]: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    (-1..=1).flat_map(move |dx| {
        (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| [x + dx, y + dy, z + dz]))
    })
}

/// Items kept in square buckets of the plate `cell_mm` across, so a search near a point
/// asks the buckets around it rather than every item (`.claude/rules/architecture.md` 7).
#[derive(Debug, Clone)]
pub struct Buckets<T> {
    cell_mm: Scalar,
    cells: FastMap<u64, Vec<T>>,
}

impl<T> Buckets<T> {
    pub fn new(cell_mm: Scalar) -> Self {
        Self {
            cell_mm,
            cells: FastMap::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// How many items are kept, counted bucket by bucket.
    pub fn len(&self) -> usize {
        self.cells.values().map(Vec::len).sum()
    }

    /// The bucket `at` falls in, as its column and row.
    pub fn cell_of(&self, at: Vec2) -> [i32; 2] {
        [
            (at.x / self.cell_mm).floor() as i32,
            (at.y / self.cell_mm).floor() as i32,
        ]
    }

    pub fn insert(&mut self, at: Vec2, item: T) {
        let key = packed(self.cell_of(at));
        self.cells.entry(key).or_default().push(item);
    }

    /// Keeps of the bucket `at` falls in only what `keep` answers true for.
    pub fn retain_at(&mut self, at: Vec2, keep: impl FnMut(&T) -> bool) {
        if let Some(bucket) = self.cells.get_mut(&packed(self.cell_of(at))) {
            bucket.retain(keep);
        }
    }

    /// Keeps of every bucket only what `keep` answers true for.
    pub fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) {
        self.cells.retain(|_, bucket| {
            bucket.retain(&mut keep);
            !bucket.is_empty()
        });
    }

    /// Everything in the bucket `at` falls in.
    pub fn at(&self, at: Vec2) -> &[T] {
        self.cells
            .get(&packed(self.cell_of(at)))
            .map_or(&[], Vec::as_slice)
    }

    /// Everything in the nine buckets around the one `at` falls in.
    pub fn around(&self, at: Vec2) -> impl Iterator<Item = &T> {
        let [x, y] = self.cell_of(at);
        self.span([x - 1, y - 1], [x + 1, y + 1])
    }

    /// Everything in the buckets the box `min`..`max` meets.
    pub fn within(&self, min: Vec2, max: Vec2) -> impl Iterator<Item = &T> {
        self.span(self.cell_of(min), self.cell_of(max))
    }

    /// Gives back what the buckets hold room for and no longer use.
    pub fn shrink_to_fit(&mut self) {
        self.cells.shrink_to_fit();
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.cells.values().flatten()
    }

    fn span(&self, low: [i32; 2], high: [i32; 2]) -> impl Iterator<Item = &T> {
        (low[1]..=high[1])
            .flat_map(move |y| (low[0]..=high[0]).map(move |x| packed([x, y])))
            .filter_map(|key| self.cells.get(&key))
            .flatten()
    }
}

impl<T: Clone> Buckets<T> {
    /// Puts `item` in every bucket the box `min`..`max` meets.
    pub fn insert_over(&mut self, min: Vec2, max: Vec2, item: &T) {
        let (low, high) = (self.cell_of(min.min(max)), self.cell_of(min.max(max)));
        for y in low[1]..=high[1] {
            for x in low[0]..=high[0] {
                let bucket = self.cells.entry(packed([x, y])).or_default();
                bucket.push(item.clone());
            }
        }
    }
}

/// A bucket as one word, which hashes in one step where a pair of integers hashes a byte
/// at a time.
fn packed([x, y]: [i32; 2]) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(y as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn around_finds_the_nine_buckets_and_nothing_further() {
        let mut buckets = Buckets::new(1.0);
        for (index, x) in [-1.5, -0.5, 0.5, 1.5, 2.5].into_iter().enumerate() {
            buckets.insert(Vec2::new(x, 0.5), index);
        }
        let mut found: Vec<usize> = buckets.around(Vec2::new(0.5, 0.5)).copied().collect();
        found.sort_unstable();
        assert_eq!(found, vec![1, 2, 3], "cells -1, 0 and 1 around cell 0");
    }

    #[test]
    fn an_item_put_over_a_box_is_found_from_each_end() {
        let mut buckets = Buckets::new(1.0);
        buckets.insert_over(Vec2::new(5.5, 0.5), Vec2::new(0.5, 0.5), &7);
        assert_eq!(buckets.at(Vec2::new(0.2, 0.2)), &[7]);
        assert_eq!(buckets.at(Vec2::new(5.9, 0.9)), &[7]);
        assert_eq!(buckets.iter().count(), 6, "one per bucket from x = 0 to 5");
    }

    #[test]
    fn retain_drops_buckets_left_empty() {
        let mut buckets = Buckets::new(1.0);
        buckets.insert(Vec2::ZERO, 1);
        buckets.retain(|item| *item != 1);
        assert!(buckets.is_empty());
    }
}
