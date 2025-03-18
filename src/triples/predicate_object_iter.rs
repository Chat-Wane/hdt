use crate::triples::Id;
use crate::triples::TriplesBitmap;
use std::cmp::Ordering;
use sucds::int_vectors::Access;

// see filterPredSubj in "Exchange and Consumption of Huge RDF Data" by Martinez et al. 2012
// https://link.springer.com/chapter/10.1007/978-3-642-30284-8_36

/// Iterator over all subject IDs with a given predicate and object ID, answering an (?S,P,O) query.
pub struct PredicateObjectIter<'a> {
    triples: &'a TriplesBitmap,
    pos_index: usize,
    max_index: usize,
}

impl<'a> PredicateObjectIter<'a> {
    /// Create a new iterator over all triples with the given predicate and object ID.
    /// Panics if the predicate or object ID is 0.
    pub fn new(triples: &'a TriplesBitmap, p: Id, o: Id) -> Self {
        assert_ne!(0, p, "predicate 0 does not exist, cant iterate");
        assert_ne!(0, o, "object 0 does not exist, cant iterate");
        let mut low = triples.op_index.find(o);
        let mut high = triples.op_index.last(o);
        let get_y = |pos_index| {
            let pos_y = triples.op_index.sequence.access(pos_index).unwrap();
            triples.wavelet_y.access(pos_y).unwrap() as Id
        };
        // Binary search with a twist:
        // Each value may occur multiple times, so we search for the left and right borders.
        while low <= high {
            let mut mid = usize::midpoint(low, high);
            match get_y(mid).cmp(&p) {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => {
                    let mut left_high = mid;
                    while low < left_high {
                        mid = usize::midpoint(low, left_high);
                        match get_y(mid).cmp(&p) {
                            Ordering::Less => low = mid + 1,
                            Ordering::Greater => {
                                high = mid;
                                left_high = mid;
                            }
                            Ordering::Equal => left_high = mid,
                        }
                    }
                    // right border
                    let mut right_low = low;
                    while right_low < high {
                        mid = (right_low + high).div_ceil(2);
                        match get_y(mid).cmp(&p) {
                            Ordering::Greater => high = mid - 1,
                            _ => right_low = mid,
                        }
                    }
                    return PredicateObjectIter { triples, pos_index: low, max_index: high };
                }
            }
            if (high == 0 && low == 0) || (high == low && high == mid) {
                break;
            }
        }
        // not found
        PredicateObjectIter { triples, pos_index: 999, max_index: 0 }
    }

    pub fn new_with_offset(triples: &'a TriplesBitmap, p: Id, o: Id, op_offset: Option<usize>) -> Self {
        match op_offset {
            None => PredicateObjectIter::new(triples, p, o),
            Some(offset) => {
                let mut base = PredicateObjectIter::new(triples, p, o);
                base.pos_index += offset; // as if we called `next` offset times
                base
            }
        }
    }
}

impl Iterator for PredicateObjectIter<'_> {
    type Item = Id;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos_index > self.max_index {
            return None;
        }
        let pos_y = self.triples.op_index.sequence.access(self.pos_index).unwrap();
        //let y = self.triples.wavelet_y.get(pos_y as usize) as Id;
        //println!(" op p {y}");
        let s = self.triples.adjlist_y.bitmap.rank(pos_y) as Id + 1;
        self.pos_index += 1;
        Some(s)
    }

    /// Provides exact cardinality on VPO
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.pos_index > self.max_index {(0, Some(0))}
        else {(self.max_index - self.pos_index + 1, Some(self.max_index - self.pos_index + 1))}
    }
}

#[cfg(test)]
mod tests {
    use crate::{Hdt, IdKind};

    #[test]
    fn skip_on_vpo() {
        let file = std::fs::File::open("/Users/skoazell/Desktop/Projects/datasets/watdiv10m-hdt/watdiv.10M.hdt").expect("error opening file");
        let hdt = Hdt::new(std::io::BufReader::new(file)).expect("error loading HDT");

        let s = "http://db.uwaterloo.ca/~galuc/wsdbm/User44276".into();
        let p = "http://db.uwaterloo.ca/~galuc/wsdbm/friendOf".into();
        let o = "http://db.uwaterloo.ca/~galuc/wsdbm/User69629".into();

        let sid = Some(hdt.dict.string_to_id(s, &IdKind::Subject));
        let pid = Some(hdt.dict.string_to_id(p, &IdKind::Predicate));
        let oid = Some(hdt.dict.string_to_id(o, &IdKind::Object));

        // VPO
        let count_vpo = hdt.triple_ids_with_pattern_and_offset(None, pid, oid, None);
        println!("vpo  estim: {:?}  vs total : {}", count_vpo.size_hint(), count_vpo.count());
        let skip_vpo = hdt.triple_ids_with_pattern_and_offset(None, pid, oid, Some(20));
        println!("skip estim: {:?}  vs actual: {}\n", skip_vpo.size_hint(), skip_vpo.count());
    }
}