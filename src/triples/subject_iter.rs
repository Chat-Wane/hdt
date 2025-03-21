use super::{Id, TripleId, TriplesBitmap};

/// Iterator over triples fitting an SPO, SP? S?? or ??? triple pattern.
//#[derive(Debug)]
pub struct SubjectIter<'a> {
    // triples data
    triples: &'a TriplesBitmap,
    // x-coordinate identifier
    x: Id,
    // current position
    pos_y: usize,
    pos_z: usize,
    max_y: usize,
    max_z: usize,
    search_z: usize, // for S?O
}

impl<'a> SubjectIter<'a> {
    /// Create an iterator over all triples.
    pub fn new(triples: &'a TriplesBitmap) -> Self {
        SubjectIter {
            triples,
            x: 1, // was 0 in the old code but it should start at 1
            pos_y: 0,
            pos_z: 0,
            max_y: triples.wavelet_y.len(), // exclusive
            max_z: triples.adjlist_z.len(), // exclusive
            search_z: 0,
        }
    }

    /// Use when no results are found.
    pub const fn empty(triples: &'a TriplesBitmap) -> Self {
        SubjectIter { triples, x: 1, pos_y: 0, pos_z: 0, max_y: 0, max_z: 0, search_z: 0 }
    }

    /// Convenience method for the S?? triple pattern.
    /// See <https://github.com/rdfhdt/hdt-cpp/blob/develop/libhdt/src/triples/BitmapTriplesIterators.cpp>.
    pub fn with_s(triples: &'a TriplesBitmap, subject_id: Id) -> Self {
        let min_y = triples.find_y(subject_id - 1);
        let min_z = triples.adjlist_z.find(min_y as Id);
        let max_y = triples.find_y(subject_id);
        let max_z = triples.adjlist_z.find(max_y as Id);
        SubjectIter { triples, x: subject_id, pos_y: min_y, pos_z: min_z, max_y, max_z, search_z: 0 }
    }

    /// Iterate over triples fitting the given SPO, SP? S??, S?O or ??? triple pattern.
    /// Variable positions are signified with a 0 value.
    /// Undefined result if any other triple pattern is used.
    /// # Examples
    /// ```text
    /// // S?? pattern, all triples with subject ID 1
    /// SubjectIter::with_pattern(triples, TripleId::new(1, 0, 0);
    /// // SP? pattern, all triples with subject ID 1 and predicate ID 2
    /// SubjectIter::with_pattern(triples, TripleId::new(1, 2, 0);
    /// // match a specific triple, not useful in practice except as an ASK query
    /// SubjectIter::with_pattern(triples, TripleId::new(1, 2, 3);
    /// ```
    // Translated from <https://github.com/rdfhdt/hdt-cpp/blob/develop/libhdt/src/triples/BitmapTriplesIterators.cpp>.
    pub fn with_pattern(triples: &'a TriplesBitmap, pat: &TripleId) -> Self {
        let (pat_x, pat_y, pat_z) = (pat.subject_id, pat.predicate_id, pat.object_id);
        let (min_y, max_y, min_z, max_z);
        let mut x = 1;
        let mut search_z = 0;
        // only SPO order is supported currently
        if pat_x != 0 {
            // S X X
            if pat_y != 0 {
                // S P X
                match triples.search_y(pat_x - 1, pat_y) {
                    Some(y) => min_y = y,
                    None => return SubjectIter::empty(triples),
                }
                max_y = min_y + 1;
                if pat_z != 0 {
                    // S P O
                    // simply with try block when they come to stable Rust
                    match triples.adjlist_z.search(min_y, pat_z) {
                        Some(pos_z) => min_z = pos_z,
                        None => return SubjectIter::empty(triples),
                    }
                    max_z = min_z + 1;
                } else {
                    // S P ?
                    min_z = triples.adjlist_z.find(min_y);
                    max_z = triples.adjlist_z.last(min_y) + 1;
                }
            } else {
                // S ? X
                min_y = triples.find_y(pat_x - 1);
                min_z = triples.adjlist_z.find(min_y);
                max_y = triples.last_y(pat_x - 1) + 1;
                max_z = triples.adjlist_z.find(max_y);
                search_z = pat_z;
            }
            x = pat_x;
        } else {
            // ? X X
            // assume ? ? ?, other triple patterns are not supported by this iterator
            min_y = 0;
            min_z = 0;
            max_y = triples.wavelet_y.len();
            max_z = triples.adjlist_z.len();
        }
        SubjectIter { triples, x, pos_y: min_y, pos_z: min_z, max_y, max_z, search_z }
    }


}

impl Iterator for SubjectIter<'_> {
    type Item = TripleId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos_y >= self.max_y {
            return None;
        }

        let y = self.triples.wavelet_y.access(self.pos_y).unwrap() as Id;

        if self.search_z > 0 {
            self.pos_y += 1;
            match self.triples.adjlist_z.search(self.pos_y - 1, self.search_z) {
                Some(_) => {
                    return Some(self.triples.coord_to_triple(self.x, y, self.search_z).unwrap());
                }
                None => {
                    return self.next();
                }
            }
        }

        if self.pos_z >= self.max_z {
            return None;
        }
        let z = self.triples.adjlist_z.get_id(self.pos_z);
        let triple_id = self.triples.coord_to_triple(self.x, y, z).unwrap();

        // theoretically the second condition should only be true if the first is as well but in practise it wasn't, which screwed up the subject identifiers
        // fixed by moving the second condition inside the first one but there may be another reason for the bug occuring in the first place
        if self.triples.adjlist_z.at_last_sibling(self.pos_z) {
            if self.triples.bitmap_y.at_last_sibling(self.pos_y) {
                self.x += 1;
            }
            self.pos_y += 1;
        }
        self.pos_z += 1;
        Some(triple_id)
    }

    /// Exact for VVV, SVV, SPV, SPO;
    /// Approximate for SVO.
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.search_z {
            0 => (self.max_z - self.pos_z, Some(self.max_z - self.pos_z)),
            _ => (0, Some(self.max_z- self.pos_z)), // not exact for SVO
        }
    }

    /// Jumps to an offset efficiently for iterators VVV, SVV, SPV.
    /// This avoids the need to iterate over every element until reaching the desired
    /// offset. (Except for SVO)
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match self.search_z {
            0 => { // Efficient for VVV, SVV, SPV
                self.pos_z += n;
                if self.pos_z > self.max_z { // prevents getting out of bound
                    self.pos_z = self.max_z;
                }
                self.pos_y = self.triples.adjlist_z.bitmap.rank(self.pos_z -1);
                self.x = self.triples.bitmap_y.rank(self.pos_y -1);
            },
            _ => { (0..n).for_each(|_| {self.next();}) } // SVO is not efficient
        }
        self.next()
    }
}


#[cfg(test)]
pub(crate) mod tests {
    use crate::triples::Id;
    use crate::{Hdt, HdtGraph, IdKind};
    use sophia::api::graph::Graph;
    use sophia::api::prelude::Any;
    use std::time::Instant;

    #[ignore]
    #[test]
    fn performance_check_on_larger_file() {
        // TODO could be downloaded conditionally
        // the file can be found on https://zenodo.org/records/13734676/files/watdiv.10M.hdt
        let file = std::fs::File::open("/tests/resources/watdiv.10M.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");
        let graph = HdtGraph::new(hdt);
        let all_triples = graph.triples_matching(Any, Any, Any);
        const SKIP: usize = 5_000_000;

        // #1 check the time of triples with their actual value
        let start_triples = Instant::now();
        let mut skipped = all_triples.skip(SKIP);
        println!("To avoid laziness, here is the element after skip: {:?}", skipped.next());
        println!("Took {:?} to skip {SKIP} elements.", start_triples.elapsed());
        let start_count = Instant::now();
        println!("We count the rest of the elements: {}", skipped.count());
        println!("Took {:?} to count the rest.", start_count.elapsed());
        // The order of magnitude is second


        // #2 check the time of counting on triple identifiers only
        let triple_ids = graph.hdt.triple_ids_with_pattern(None, None, None);
        let start_count_triples_ids = Instant::now();
        println!("We count the number of elements {}.", triple_ids.count());
        println!("Took {:?} to count.", start_count_triples_ids.elapsed());
        // The order of magnitude is milliseconds!

        // #3 skip on triple identifiers
        let start_triple_ids = Instant::now();
        let mut triples_ids_to_skip = graph.hdt.triple_ids_with_pattern(None, None, None).skip(SKIP);
        println!("To avoid laziness, here is the element after skip: {:?}", triples_ids_to_skip.next());
        println!("Took {:?} to skip {SKIP} elements.", start_triple_ids.elapsed());
        // The order of magnitude is microsecond!!
        let start_count_rest_ids = Instant::now();
        println!("We count the rest of the elements: {}", triples_ids_to_skip.count());
        println!("Took {:?} to count the rest.", start_count_rest_ids.elapsed());
        // The rest is half the time of previous count obviously
    }

    #[ignore]
    #[test]
    fn skip_on_subject_iterators() {
        // TODO again, could be downloaded conditionally
        let file = std::fs::File::open("/tests/resources/watdiv.10M.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        let s = "http://db.uwaterloo.ca/~galuc/wsdbm/User44276".into();
        let p = "http://db.uwaterloo.ca/~galuc/wsdbm/friendOf".into();
        let o = "http://db.uwaterloo.ca/~galuc/wsdbm/User69629".into();

        let sid = Some(hdt.dict.string_to_id(s, &IdKind::Subject));
        let pid = Some(hdt.dict.string_to_id(p, &IdKind::Predicate));
        let oid = Some(hdt.dict.string_to_id(o, &IdKind::Object));

        let count_vvv = hdt.triple_ids_with_pattern(None, None, None);
        println!("vvv  estim: {:?}  vs total : {}", count_vvv.size_hint(), count_vvv.count());
        let skip_vvv = hdt.triple_ids_with_pattern(None, None, None).skip(5_000_000);
        println!("skip estim: {:?}  vs actual: {}\n", skip_vvv.size_hint(), skip_vvv.count());

        let count_svv = hdt.triple_ids_with_pattern(sid, None, None);
        println!("svv  estim: {:?} vs total : {}", count_svv.size_hint(), count_svv.count());
        let skip_svv = hdt.triple_ids_with_pattern(sid, None, None).skip(20);
        println!("skip estim: {:?} vs actual: {} \n", skip_svv.size_hint(), skip_svv.count());

        let count_spv = hdt.triple_ids_with_pattern(sid, pid, None);
        println!("spv estim : {:?} vs total : {}", count_spv.size_hint(), count_spv.count());
        let skip_spv = hdt.triple_ids_with_pattern(sid, pid, None).skip(150);
        println!("skip estim: {:?} vs actual: {}\n", skip_spv.size_hint(), skip_spv.count());

        let count_svo = hdt.triple_ids_with_pattern(sid, None, oid);
        println!("svo estim : {:?} vs total : {}", count_svo.size_hint(), count_svo.count());
        let skip_svo = hdt.triple_ids_with_pattern(sid, None, oid).skip(1);
        println!("skip estim: {:?} vs actual: {}\n", skip_svo.size_hint(), skip_svo.count());

        let count_spo = hdt.triple_ids_with_pattern(sid, pid, oid);
        println!("spo estim : {:?} vs total : {}", count_spo.size_hint(), count_spo.count());
        let skip_spo = hdt.triple_ids_with_pattern(sid, pid, oid).skip(1);
        println!("skip estim: {:?} vs actual: {}", skip_spo.size_hint(), skip_spo.count());
    }

    #[test]
    fn skip_on_vvv_should_count_the_remaining_and_cardinality_is_exact() {
        let file = std::fs::File::open("tests/resources/snikmeta.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        assert_exact_cardinality_of_pattern(&hdt, None, None, None);
        // 328 triples in the dataset!
        assert_number_of_remaining_elements_after_skip(&hdt, None, None, None, 000);
        assert_number_of_remaining_elements_after_skip(&hdt, None, None, None, 100);
        assert_number_of_remaining_elements_after_skip(&hdt, None, None, None, 200);
        assert_number_of_remaining_elements_after_skip(&hdt, None, None, None, 300);
        assert_number_of_remaining_elements_after_skip(&hdt, None, None, None, 100_000);
    }

    #[test]
    fn skip_on_svv_should_count_the_remaining_and_cardinality_is_exact() {
        let file = std::fs::File::open("tests/resources/snikmeta.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        let s1 = "http://www.snik.eu/ontology/meta".into();
        let s2 = "http://www.snik.eu/ontology/meta/ApplicationComponent".into();
        let s3 = "http://www.snik.eu/ontology/meta/book".into();

        let sid1 = Some(hdt.dict.string_to_id(s1, &IdKind::Subject));
        let sid2 = Some(hdt.dict.string_to_id(s2, &IdKind::Subject));
        let sid3 = Some(hdt.dict.string_to_id(s3, &IdKind::Subject));

        assert_exact_cardinality_of_pattern(&hdt, sid1, None, None); // 65 triples
        assert_exact_cardinality_of_pattern(&hdt, sid2, None, None); // 3 triples
        assert_exact_cardinality_of_pattern(&hdt, sid3, None, None); // 6 triples

        assert_number_of_remaining_elements_after_skip(&hdt, sid1, None, None, 10);
        assert_number_of_remaining_elements_after_skip(&hdt, sid2, None, None, 12);
        assert_number_of_remaining_elements_after_skip(&hdt, sid3, None, None, 1);
    }

    #[test]
    fn skip_on_spv_should_count_the_remaining_and_cardinality_is_exact() {
        let file = std::fs::File::open("tests/resources/snikmeta.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        let s1 = "http://www.snik.eu/ontology/meta".into();
        let p1 = "http://open.vocab.org/terms/defines".into();
        let p2 = "http://purl.org/dc/terms/creator".into();

        let sid1 = Some(hdt.dict.string_to_id(s1, &IdKind::Subject));
        let pid1 = Some(hdt.dict.string_to_id(p1, &IdKind::Predicate));
        let pid2 = Some(hdt.dict.string_to_id(p2, &IdKind::Predicate));

        assert_exact_cardinality_of_pattern(&hdt, sid1, pid1, None); // 44 triples
        assert_exact_cardinality_of_pattern(&hdt, sid1, pid2, None); // 7 triples

        assert_number_of_remaining_elements_after_skip(&hdt, sid1, pid1, None, 10);
        assert_number_of_remaining_elements_after_skip(&hdt, sid1, pid1, None, 20);
        assert_number_of_remaining_elements_after_skip(&hdt, sid1, pid2, None, 2);
        assert_number_of_remaining_elements_after_skip(&hdt, sid1, pid2, None, 1000);
    }

    #[test]
    fn skip_on_svo_not_efficient_nor_cardinality_exact_but_we_have_higher_bound() {
        let file = std::fs::File::open("tests/resources/snikmeta.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        let s = "http://www.snik.eu/ontology/meta".into();
        let o = "http://www.snik.eu".into();
        let sid = Some(hdt.dict.string_to_id(s, &IdKind::Subject));
        let oid = Some(hdt.dict.string_to_id(o, &IdKind::Object));

        assert_consistent_cardinality_of_pattern(&hdt, sid, None, oid); // 3 triples
        // still checking on skip even though it's not efficient
        assert_number_of_remaining_elements_after_skip(&hdt, sid, None, oid, 0);
        assert_number_of_remaining_elements_after_skip(&hdt, sid, None, oid, 2);
        assert_number_of_remaining_elements_after_skip(&hdt, sid, None, oid, 10);
    }

    /// Checks if the cardinality estimation on the targeted triple pattern provides the exact
    /// number of elements of this pattern in the hdt database.
    pub fn assert_exact_cardinality_of_pattern(hdt: &Hdt, s: Option<Id>, p: Option<Id>, o: Option<Id>) {
        let count_it = hdt.triple_ids_with_pattern(s, p, o);
        let estimate = count_it.size_hint();
        let actual_count = count_it.count();
        assert_eq!(actual_count, estimate.0);
        assert_eq!(actual_count, estimate.1.unwrap());
    }

    /// Checks if the cardinality estimation is consistent with the actual
    /// number of triples for the triple pattern.
    pub fn assert_consistent_cardinality_of_pattern(hdt: &Hdt, s: Option<Id>, p: Option<Id>, o: Option<Id>) {
        let count_it = hdt.triple_ids_with_pattern(s, p, o);
        let estimate = count_it.size_hint();
        let actual_count = count_it.count();
        assert!(estimate.0 <= actual_count);
        match estimate.1 {
            Some(higher) => assert!(actual_count <= higher),
            None => {},
        }
    }

    /// Checks if the number of element after skip is consistent with the actual number
    /// of elements in the targeted pattern.
    pub fn assert_number_of_remaining_elements_after_skip(hdt: &Hdt, s: Option<Id>, p: Option<Id>, o: Option<Id>, skip: usize) {
        let count_it = hdt.triple_ids_with_pattern(s, p, o);
        let actual_count = count_it.count();
        let start = Instant::now();
        let skipped_it = hdt.triple_ids_with_pattern(s, p, o).skip(skip);
        let count_after_skip = skipped_it.count();
        if skip < actual_count {
            assert!(skip < actual_count && count_after_skip > 0); // just to make sure that we actually skip something
            assert_eq!(actual_count, count_after_skip + skip);
        } else {
            assert_eq!(0, count_after_skip);
        }
    }

}