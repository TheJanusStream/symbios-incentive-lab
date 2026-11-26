//! Live posts in id order. Posts leave the store after their lifetime and their
//! final reward and payout accounting, so memory stays bounded over long runs.

use std::collections::VecDeque;

use crate::content::Post;

/// The live posts, oldest first, with contiguous ids.
#[derive(Clone, Debug, Default)]
pub(crate) struct PostStore {
    posts: VecDeque<Post>,
    first_id: u32,
    next_id: u32,
}

impl PostStore {
    /// The id the next post gets.
    pub(crate) fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Smallest live id; every older post has been pruned.
    pub(crate) fn first_id(&self) -> u32 {
        self.first_id
    }

    /// Adds a post whose id must be [`PostStore::next_id`].
    pub(crate) fn push(&mut self, post: Post) {
        debug_assert_eq!(post.id, self.next_id);
        self.next_id += 1;
        self.posts.push_back(post);
    }

    /// Position of a live post.
    pub(crate) fn slot(&self, id: u32) -> Option<usize> {
        let slot = id.checked_sub(self.first_id)? as usize;
        (slot < self.posts.len()).then_some(slot)
    }

    pub(crate) fn get(&self, id: u32) -> Option<&Post> {
        self.slot(id).and_then(|s| self.posts.get(s))
    }

    pub(crate) fn get_mut(&mut self, id: u32) -> Option<&mut Post> {
        let slot = self.slot(id)?;
        self.posts.get_mut(slot)
    }

    pub(crate) fn len(&self) -> usize {
        self.posts.len()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Post> {
        self.posts.iter()
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Post> {
        self.posts.iter_mut()
    }

    /// Live posts with ids from `first` on (today's posts, for example).
    pub(crate) fn iter_from_mut(&mut self, first: u32) -> impl Iterator<Item = &mut Post> {
        let skip = first.saturating_sub(self.first_id) as usize;
        self.posts.iter_mut().skip(skip)
    }

    /// Removes the oldest posts while `expired` holds for them.
    pub(crate) fn prune_front(&mut self, mut expired: impl FnMut(&Post) -> bool) {
        while self.posts.front().is_some_and(&mut expired) {
            self.posts.pop_front();
            self.first_id += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{AuthorProfile, Group, Style, generate_post};
    use crate::rng::{self, Stream};

    #[test]
    fn ids_stay_addressable_after_pruning() {
        let mut rng = rng::stream_rng(1, 0, 0, Stream::Posting);
        let author = AuthorProfile::plain(Group::A, 0.5, 0.2);
        let mut store = PostStore::default();
        for day in 0..4 {
            let id = store.next_id();
            store.push(generate_post(
                &mut rng,
                id,
                day,
                &author,
                Style::Personal,
                &[1.0 / 6.0; 6],
                &crate::config::WorldAssumptions::default(),
            ));
        }
        store.prune_front(|p| p.day < 2);
        assert_eq!(store.len(), 2);
        assert!(store.get(1).is_none());
        assert_eq!(store.get(3).map(|p| p.day), Some(3));
    }
}
