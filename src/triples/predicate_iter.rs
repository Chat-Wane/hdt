use crate::triples::Id;
use crate::triples::TripleId;
use crate::triples::TriplesBitmap;

/// Iterator over all triples with a given property ID, answering an (?S,P,?O) query.
pub struct PredicateIter<'a> {
    triples: &'a TriplesBitmap,
    s: Id,
    p: Id,
    i: usize,
    os: usize,
    pos_z: usize,
    occs: usize,
}

impl<'a> PredicateIter<'a> {
    /// Create a new iterator over all triples with the given property ID.
    /// Panics if the object does not exist.
    pub fn new(triples: &'a TriplesBitmap, p: Id) -> Self {
        assert!(p != 0, "object 0 does not exist, cant iterate");
        let occs = triples.wavelet_y.rank(triples.wavelet_y.len(), p as usize).unwrap();
        //println!("the predicate {} is used by {} subjects in the index", p, occs);
        PredicateIter { triples, p, i: 0, pos_z: 0, os: 0, s: 0, occs }
    }
}


impl Iterator for PredicateIter<'_> {
    type Item = TripleId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.occs {
            return None;
        }
        if self.os == 0 {
            // Algorithm 1 findSubj from Martinez et al. 2012 ******
            let pos_y = self.triples.wavelet_y.select(self.i, self.p as usize).unwrap();
            self.s = self.triples.bitmap_y.rank(pos_y) as Id + 1;
            // *****************************************************
            // SP can have multiple O
            self.pos_z = self.triples.adjlist_z.find(pos_y as Id);
            let pos_z_end = self.triples.adjlist_z.last(pos_y as Id);
            //println!("**** found predicate {} between {} and {} (inclusive)", self.p, self.pos_z, pos_z_end);
            self.os = pos_z_end - self.pos_z;
        } else {
            self.os -= 1;
            self.pos_z += 1;
        }

        let o = self.triples.adjlist_z.sequence.get(self.pos_z) as Id;
        if self.os == 0 {
            self.i += 1;
        }
        Some(self.triples.coord_to_triple(self.s, self.p, o).unwrap())
    }

    /// Only the lower bound is known for VPV
    fn size_hint(&self) -> (usize, Option<usize>) {
        // but for each occs, there might have multiple values
        // so we don't know the upper bound.
        (self.occs, None)
    }

    // Iterator for VPV that optionally skips to the offset. However,
    // there are no efficient way to jump immediately to the designated offset.
    // so `fn nth` not override, since the default is the actual best implem'.
}


#[cfg(test)]
mod tests {
    use crate::triples::subject_iter::tests::{assert_consistent_cardinality_of_pattern, assert_number_of_remaining_elements_after_skip};
    use crate::{Hdt, IdKind};

    #[ignore]
    #[test]
    fn skip_on_vpv_on_larger_file() {
        // TODO could be downloaded conditionally
        // the file can be found on https://zenodo.org/records/13734676/files/watdiv.10M.hdt
        let file = std::fs::File::open("/tests/resources/watdiv.10M.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        let p = "http://db.uwaterloo.ca/~galuc/wsdbm/friendOf".into();
        let pid = Some(hdt.dict.string_to_id(p, &IdKind::Predicate));

        // VPV
        let count_vpv = hdt.triple_ids_with_pattern(None, pid, None);
        println!("vpv  estim: {:?}  vs total : {}", count_vpv.size_hint(), count_vpv.count());
        let skip_vpv = hdt.triple_ids_with_pattern(None, pid, None).skip(2_000_000);
        println!("skip estim: {:?}  vs actual: {}\n", skip_vpv.size_hint(), skip_vpv.count());
    }

    #[test]
    fn skip_on_vpv_not_efficient_nor_exact_cardinality() {
        let file = std::fs::File::open("tests/resources/snikmeta.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        let p = "http://www.w3.org/2000/01/rdf-schema#range".into();
        let pid = Some(hdt.dict.string_to_id(p, &IdKind::Predicate));

        assert_consistent_cardinality_of_pattern(&hdt, None, pid, None); // 33 triples
        // still checking skip, even though not efficient
        assert_number_of_remaining_elements_after_skip(&hdt, None, pid, None, 0);
        assert_number_of_remaining_elements_after_skip(&hdt, None, pid, None, 10);
        assert_number_of_remaining_elements_after_skip(&hdt, None, pid, None, 1000);
    }
}