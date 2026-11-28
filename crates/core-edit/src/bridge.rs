/// Two open edges written `A-B:C-D` by vertex numbers, as `Edit::Mend`'s `bridge` takes them;
/// `None` for text not so written.
pub fn bridge_edges(text: &str) -> Option<[[u32; 2]; 2]> {
    let edge = |part: &str| -> Option<[u32; 2]> {
        let (a, b) = part.split_once('-')?;
        Some([a.trim().parse().ok()?, b.trim().parse().ok()?])
    };
    let (first, second) = text.split_once(':')?;
    Some([edge(first)?, edge(second)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_edges_read_as_their_vertex_numbers() {
        assert_eq!(bridge_edges("1-2: 5 -6"), Some([[1, 2], [5, 6]]));
    }

    #[test]
    fn one_edge_or_a_word_is_no_bridge() {
        assert_eq!(bridge_edges("1-2"), None);
        assert_eq!(bridge_edges("1-x:5-6"), None);
    }
}
