use sv_parser::RefNode;

#[derive(Clone, Debug)]
pub enum MatchPattern<'a> {
    Matches(&'a str),
    NotMatches(&'a str),
}

pub trait TryIntoLocate {
    fn try_into_locate(&self) -> Option<&sv_parser::Locate>;
}

impl TryIntoLocate for RefNode<'_> {
    fn try_into_locate(&self) -> Option<&sv_parser::Locate> {
        if let RefNode::Locate(locate) = self {
            Some(locate)
        } else {
            None
        }
    }
}

impl TryIntoLocate for &str {
    fn try_into_locate(&self) -> Option<&sv_parser::Locate> {
        None
    }
}

pub struct BreadcrumbsMatcher<'a> {
    pattern: Vec<MatchPattern<'a>>,
    current_match: usize,
    current_notmatch: Vec<usize>,
    callback: Box<dyn Fn(&sv_parser::Locate) + 'a>,
}

impl<'a> BreadcrumbsMatcher<'a> {
    pub fn new(
        nodes: Vec<MatchPattern<'a>>,
        callback: Box<dyn Fn(&sv_parser::Locate) + 'a>,
    ) -> Self {
        Self {
            pattern: nodes.to_vec().clone(),
            current_match: 0,
            current_notmatch: vec![],
            callback,
        }
    }
    pub fn enter<T>(&mut self, node: &T)
    where
        T: TryIntoLocate + ToString,
    {
        #[cfg(test)]
        println!(
            "ENTER: {:?}, {}, {:?}, {}",
            self.pattern,
            self.current_match,
            self.current_notmatch,
            node.to_string()
        );

        let mut local_match_counter = self.current_match;
        let node_tostring = node.to_string();

        while let Some(MatchPattern::NotMatches(patt)) = self.pattern.get(local_match_counter) {
            if *patt == node_tostring {
                self.current_notmatch.push(local_match_counter);
            }
            local_match_counter += 1;
        }

        #[cfg(test)]
        println!(
            "LOOP : {:?}, {}, {:?}",
            self.pattern.get(local_match_counter),
            local_match_counter,
            self.current_notmatch
        );

        match self.pattern.get(local_match_counter) {
            None => {
                if let Some(locate) = node.try_into_locate() {
                    (self.callback)(locate);
                }
            }
            Some(MatchPattern::Matches(patt)) => {
                if *patt == node_tostring {
                    self.current_match = local_match_counter + 1;
                    #[cfg(test)]
                    println!(
                        "Matched? {} == {} and {:?}",
                        self.current_match,
                        self.pattern.len(),
                        self.current_notmatch
                    );
                    if self.matches() {
                        if let Some(locate) = node.try_into_locate() {
                            (self.callback)(locate);
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn leave<T>(&mut self, node: &T)
    where
        T: ToString,
    {
        #[cfg(test)]
        println!(
            "LEAVE: {:?}, {}, {:?}, {}",
            self.pattern,
            self.current_match,
            self.current_notmatch,
            node.to_string()
        );

        if self.current_match == 0 {
            assert!(
                self.current_notmatch.len() == 0,
                "Notmatch needs to be empty at level 0"
            );
            return;
        }

        let node_tostring = node.to_string();

        match self.pattern.get(self.current_match - 1) {
            None => return,
            Some(MatchPattern::Matches(patt)) => {
                if *patt == node_tostring {
                    self.current_match -= 1;
                    #[cfg(test)]
                    println!(
                        "Leave Not Matched? {} == {} and {:?}",
                        self.current_match,
                        self.pattern.len(),
                        self.current_notmatch
                    );
                }
            }
            Some(MatchPattern::NotMatches(_)) => (),
        }

        if self.current_match == 0 {
            return;
        }

        while let Some(MatchPattern::NotMatches(p)) = self.pattern.get(self.current_match - 1) {
            #[cfg(test)]
            println!(
                "LOOP : {:?}, {}, {:?}",
                self.pattern.get(self.current_match - 1),
                self.current_match,
                self.current_notmatch
            );
            if let Some(n) = self.current_notmatch.last() {
                if *n == self.current_match - 1 {
                    if *p != node_tostring {
                        return;
                    } else {
                        self.current_notmatch.pop();
                    }
                }
            }
            self.current_match -= 1;
            if self.current_match == 0 {
                assert!(
                    self.current_notmatch.len() == 0,
                    "Notmatch needs to be empty at level 0"
                );
                return;
            }
        }
    }

    pub fn matches(&self) -> bool {
        return self.current_match == self.pattern.len() && self.current_notmatch.len() == 0;
    }
}

mod tests {
    use super::*;

    #[cfg(test)]
    enum Event<'a> {
        Enter(&'a str),
        Leave(&'a str),
    }

    #[test]
    fn matcher_test_notmatch_without_next() {
        let stim: Vec<(bool, Event)> = vec![
            (false, Event::Enter("Prebase")),
            (false, Event::Enter("Base")),
            (false, Event::Enter("NextLevel")),
            (false, Event::Enter("DisallowedLevel")),
            (false, Event::Enter("NotMatched")),
            (false, Event::Leave("NotMatched")),
            (false, Event::Leave("DisallowedLevel")),
            (false, Event::Leave("Base")),
            (false, Event::Leave("Prebase")),
        ];

        let pattern = vec![
            MatchPattern::Matches("Base"),
            MatchPattern::NotMatches("DisallowedLevel"),
            MatchPattern::Matches("Something"),
        ];

        let mut bc = BreadcrumbsMatcher::new(pattern, Box::new(|_| {}));

        for (matches, event) in stim {
            match event {
                Event::Enter(e) => bc.enter(&e),
                Event::Leave(e) => bc.leave(&e),
            }
            assert!(
                bc.matches() == matches,
                "Incorrectly computed matched/mismatch"
            );
        }
    }

    #[test]
    fn matcher_notmatch_multiple() {
        let stim: Vec<(bool, Event)> = vec![
            (false, Event::Enter("Base")),
            (false, Event::Enter("DisallowedLevel")),
            (false, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Leave("DisallowedLevel")),
            (false, Event::Leave("Base")),
            (false, Event::Enter("Base")),
            (true, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Leave("Base")),
        ];

        let pattern = vec![
            MatchPattern::Matches("Base"),
            MatchPattern::NotMatches("DisallowedLevel"),
            MatchPattern::Matches("Something"),
        ];

        let mut bc = BreadcrumbsMatcher::new(pattern, Box::new(|_| {}));

        for (matches, event) in stim {
            match event {
                Event::Enter(e) => bc.enter(&e),
                Event::Leave(e) => bc.leave(&e),
            }
            assert!(
                bc.matches() == matches,
                "Incorrectly computed matched/mismatch"
            );
        }
    }

    #[test]
    fn matcher_notmatch_after() {
        let stim: Vec<(bool, Event)> = vec![
            (false, Event::Enter("Base")),
            (true, Event::Enter("Something")),
            (true, Event::Enter("DisallowedLevel")),
            (true, Event::Leave("DisallowedLevel")),
            (false, Event::Leave("Something")),
            (false, Event::Leave("Base")),
        ];

        let pattern = vec![
            MatchPattern::Matches("Base"),
            MatchPattern::NotMatches("DisallowedLevel"),
            MatchPattern::Matches("Something"),
        ];

        let mut bc = BreadcrumbsMatcher::new(pattern, Box::new(|_| {}));

        for (matches, event) in stim {
            match event {
                Event::Enter(e) => bc.enter(&e),
                Event::Leave(e) => bc.leave(&e),
            }
            assert!(
                bc.matches() == matches,
                "Incorrectly computed matched/mismatch"
            );
        }
    }

    #[test]
    fn matcher_respects_not_equal_match_reentry() {
        let stim: Vec<(bool, Event)> = vec![
            (false, Event::Enter("Base")),
            (false, Event::Enter("NextLevel")),
            (false, Event::Enter("AllowedLevel")),
            (true, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (true, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Enter("DisallowedLevel")),
            (false, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Leave("DisallowedLevel")),
            (false, Event::Leave("AllowedLevel")),
            (false, Event::Leave("NextLevel")),
            (false, Event::Leave("Base")),
            //Now we redo the above
            (false, Event::Enter("Base")),
            (false, Event::Enter("NextLevel")),
            (false, Event::Enter("AllowedLevel")),
            (true, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (true, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Enter("DisallowedLevel")),
            (false, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Leave("DisallowedLevel")),
            (false, Event::Leave("AllowedLevel")),
            (false, Event::Leave("NextLevel")),
            (false, Event::Leave("Base")),
        ];

        let pattern = vec![
            MatchPattern::Matches("Base"),
            MatchPattern::NotMatches("DisallowedLevel"),
            MatchPattern::Matches("Something"),
        ];

        let mut bc = BreadcrumbsMatcher::new(pattern, Box::new(|_| {}));

        for (matches, event) in stim {
            match event {
                Event::Enter(e) => bc.enter(&e),
                Event::Leave(e) => bc.leave(&e),
            }
            assert!(
                bc.matches() == matches,
                "Incorrectly computed matched/mismatch"
            );
        }
    }

    #[test]
    fn matcher_test_ignores_nomatch() {
        let stim: Vec<(bool, Event)> = vec![
            (false, Event::Enter("Base")),
            (true, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Leave("Base")),
        ];

        let pattern = vec![
            MatchPattern::Matches("Base"),
            MatchPattern::NotMatches("DisallowedLevel"),
            MatchPattern::Matches("Something"),
        ];

        let mut bc = BreadcrumbsMatcher::new(pattern, Box::new(|_| {}));

        for (matches, event) in stim {
            match event {
                Event::Enter(e) => bc.enter(&e),
                Event::Leave(e) => bc.leave(&e),
            }
            assert!(
                bc.matches() == matches,
                "Incorrectly computed matched/mismatch"
            );
        }
    }

    #[test]
    fn matcher_respects_not_equal_match() {
        let stim: Vec<(bool, Event)> = vec![
            (false, Event::Enter("Base")),
            (false, Event::Enter("NextLevel")),
            (false, Event::Enter("AllowedLevel")),
            (true, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Leave("AllowedLevel")),
        ];

        let pattern = vec![
            MatchPattern::Matches("Base"),
            MatchPattern::NotMatches("DisallowedLevel"),
            MatchPattern::Matches("Something"),
        ];

        let mut bc = BreadcrumbsMatcher::new(pattern, Box::new(|_| {}));

        for (matches, event) in stim {
            match event {
                Event::Enter(e) => bc.enter(&e),
                Event::Leave(e) => bc.leave(&e),
            }
            assert!(
                bc.matches() == matches,
                "Incorrectly computed matched/mismatch"
            );
        }
    }

    #[test]
    fn matcher_respects_not_equal_notmatch() {
        let stim: Vec<(bool, Event)> = vec![
            (false, Event::Enter("Base")),
            (false, Event::Enter("NextLevel")),
            (false, Event::Enter("DisallowedLevel")),
            (false, Event::Enter("Something")),
            (false, Event::Leave("Something")),
            (false, Event::Leave("DisallowedLevel")),
        ];

        let pattern = vec![
            MatchPattern::Matches("Base"),
            MatchPattern::NotMatches("DisallowedLevel"),
            MatchPattern::Matches("Something"),
        ];

        let mut bc = BreadcrumbsMatcher::new(pattern, Box::new(|_| {}));

        for (matches, event) in stim {
            match event {
                Event::Enter(e) => bc.enter(&e),
                Event::Leave(e) => bc.leave(&e),
            }
            assert!(
                bc.matches() == matches,
                "Incorrectly computed matched/mismatch"
            );
        }
    }
}
