use core_geometry::Scalar;

/// How much of the model stands on each piece of the layers a run has climbed, known once
/// the whole stack has been read.
///
/// Pieces are followed up the stack into what grows out of them. A piece that is the only
/// child of its only parent stays on that parent's node, so the graph grows with where
/// parts split and join rather than with the layers; see `docs/design/supports.md`.
pub(crate) struct Load {
    nodes: Vec<Node>,
    /// The node each piece of the layer last climbed to is on.
    of_piece: Vec<u32>,
    /// The resin one cell of one layer holds, cubic millimetres.
    cell_mm3: f64,
}

/// A run of layers over which one piece neither split nor joined another.
struct Node {
    own_mm3: f64,
    /// The nodes it grew out of, every one of them made before it.
    parents: Vec<u32>,
}

/// Where on the graph a contact was put: its node, and how much of that node lay under
/// the contact's layer and the layer itself.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Mark {
    node: u32,
    under_mm3: f64,
}

impl Load {
    pub(crate) fn new(cell_mm3: Scalar) -> Self {
        Self {
            nodes: Vec::new(),
            of_piece: Vec::new(),
            cell_mm3: f64::from(cell_mm3),
        }
    }

    /// Takes on the next layer up: what each of its pieces grew out of on the layer before,
    /// or `None` for the first layer, and how many cells each piece covers.
    pub(crate) fn climb(&mut self, parents: Option<&[Vec<u32>]>, cells: &[u64]) {
        let of_piece: Vec<u32> = match parents {
            None => cells.iter().map(|_| self.node(Vec::new())).collect(),
            Some(parents) => {
                let mut children = vec![0u32; self.of_piece.len()];
                for parent in parents.iter().flatten() {
                    children[*parent as usize] += 1;
                }
                parents
                    .iter()
                    .map(|grown_from| match grown_from.as_slice() {
                        [only] if children[*only as usize] == 1 => self.of_piece[*only as usize],
                        _ => {
                            let nodes = grown_from
                                .iter()
                                .map(|parent| self.of_piece[*parent as usize])
                                .collect();
                            self.node(nodes)
                        }
                    })
                    .collect()
            }
        };
        for (node, cells) in of_piece.iter().zip(cells) {
            self.nodes[*node as usize].own_mm3 += *cells as f64 * self.cell_mm3;
        }
        self.of_piece = of_piece;
    }

    fn node(&mut self, parents: Vec<u32>) -> u32 {
        self.nodes.push(Node {
            own_mm3: 0.0,
            parents,
        });
        (self.nodes.len() - 1) as u32
    }

    /// Where a contact put under `piece` of the layer last climbed to sits on the graph.
    pub(crate) fn mark(&self, piece: u32) -> Option<Mark> {
        let node = *self.of_piece.get(piece as usize)?;
        Some(Mark {
            node,
            under_mm3: self.nodes[node as usize].own_mm3,
        })
    }

    /// Everything standing on each node: its own resin and a share of every node that grew
    /// out of it, split evenly where one grew out of several so nothing is counted twice.
    pub(crate) fn totals(&self) -> Vec<f64> {
        let mut total: Vec<f64> = self.nodes.iter().map(|node| node.own_mm3).collect();
        // A node is made after every node it grew out of, so walking back down the list
        // finishes each one before it is handed on.
        for (index, node) in self.nodes.iter().enumerate().rev() {
            if node.parents.is_empty() {
                continue;
            }
            let share = total[index] / node.parents.len() as f64;
            for parent in &node.parents {
                total[*parent as usize] += share;
            }
        }
        total
    }
}

impl Mark {
    /// How much of the model stands above the contact's layer, cubic millimetres, given the
    /// [`Load::totals`] of the run.
    pub(crate) fn above_mm3(self, totals: &[f64]) -> Scalar {
        (totals[self.node as usize] - self.under_mm3).max(0.0) as Scalar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A graph begun with one layer of `cells`, one cubic millimetre a cell.
    fn begun(cells: &[u64]) -> Load {
        let mut load = Load::new(1.0);
        load.climb(None, cells);
        load
    }

    #[test]
    fn a_column_is_one_node_whatever_its_height() {
        let mut load = begun(&[4]);
        for _ in 0..1000 {
            load.climb(Some(&[vec![0]]), &[4]);
        }
        assert_eq!(load.nodes.len(), 1, "a piece that never splits is one node");
    }

    #[test]
    fn what_stands_above_a_contact_is_the_rest_of_its_part() {
        // Two layers of 4, then a split into 3 and 5, then 3 and 5 again.
        let mut load = begun(&[4]);
        load.climb(Some(&[vec![0]]), &[4]);
        let low = load.mark(0).expect("one piece");
        load.climb(Some(&[vec![0], vec![0]]), &[3, 5]);
        let left = load.mark(0).expect("two pieces");
        load.climb(Some(&[vec![0], vec![1]]), &[3, 5]);
        let totals = load.totals();
        assert_eq!(
            low.above_mm3(&totals),
            16.0,
            "both arms above the second layer"
        );
        assert_eq!(left.above_mm3(&totals), 3.0, "the left arm's last layer");
    }

    #[test]
    fn a_piece_grown_out_of_two_is_shared_between_them() {
        // Two posts of 2 joined by a slab of 10.
        let mut load = begun(&[2, 2]);
        load.climb(Some(&[vec![0, 1]]), &[10]);
        let totals = load.totals();
        assert_eq!(totals[0], 7.0, "a post and half the slab");
        assert_eq!(totals[1], 7.0);
    }
}
