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
    current_notmatch: Option<usize>,
    callback: Box<dyn Fn(&sv_parser::Locate)>,
}

impl<'a> BreadcrumbsMatcher<'a> {
    pub fn new(nodes: Vec<MatchPattern<'a>>, callback: Box<dyn Fn(&sv_parser::Locate)>) -> Self {
        Self {
            pattern: nodes.to_vec().clone(),
            current_match: 0,
            current_notmatch: None,
            callback,
        }
    }
    pub fn enter<T>(&mut self, node: &T)
    where
        T: TryIntoLocate + ToString,
    {
        #[cfg(test)]
        {
            println!(
                "{:?}, {}, {:?}, {}",
                self.pattern,
                self.current_match,
                self.current_notmatch,
                node.to_string()
            );
        }
        //We are already matching. Only call the callback if we're not 'strict'w
        if self.current_match == self.pattern.len() {
            return;
        }
        let current_pattern = self.pattern.get(self.current_match).unwrap();
        match current_pattern {
            MatchPattern::Matches(s) => {
                if *s == node.to_string() {
                    self.current_match += 1;
                    //If we have matched the whole list
                    if self.current_match == self.pattern.len() && self.current_notmatch.is_none() {
                        if let Some(locate) = node.try_into_locate() {
                            (self.callback)(&locate);
                        }
                    }
                }
            }
            MatchPattern::NotMatches(s) => {
                if *s != node.to_string() {
                    self.current_match += 1;
                    //If we have not matched the whole list
                    if self.current_match != self.pattern.len() {
                        self.enter(node);
                        return;
                    } else {
                        if self.current_notmatch.is_none() {
                            if let Some(locate) = node.try_into_locate() {
                                (self.callback)(&locate);
                            }
                        }
                    }
                } else {
                    self.current_notmatch.get_or_insert(self.current_match);
                }
            }
        }
        return;
    }

    pub fn leave<T>(&mut self, node: &T)
    where
        T: ToString,
    {
        if self.current_match == 0 {
            return;
        }
        if let Some(n) = self.current_notmatch {
            if n >= self.current_match {
                self.current_notmatch = None
            }
        }
        let previous_index = (self.current_match - 1) as usize;
        let current_pattern = self.pattern.get(previous_index).unwrap();
        match current_pattern {
            MatchPattern::Matches(s) => {
                if *s == node.to_string() {
                    self.current_match -= 1;
                    //If we have matched the whole list
                    if self.current_match > 0 {
                        self.leave(node);
                        return;
                    }
                }
            }
            MatchPattern::NotMatches(s) => {
                if *s != node.to_string() {
                    self.current_match -= 1;
                    //If we have matched the whole list
                    if self.current_match > 0 {
                        self.leave(node);
                        return;
                    }
                }
            }
        }
        return;
    }

    pub fn matches(&self) -> bool {
        return self.current_match == self.pattern.len() && self.current_notmatch.is_none();
    }
}
