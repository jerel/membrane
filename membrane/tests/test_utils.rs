use pretty_assertions::assert_eq;
use std::fmt;

pub fn assert_contains_part(left: &str, right: &str) {
  let left_no_ws = left.split_whitespace().collect::<String>();
  let right_no_ws = right.split_whitespace().collect::<String>();
  if !left_no_ws.contains(&right_no_ws) {
    assert_eq!(
      PrettyString(left),
      PrettyString(right),
      "\n\nThe left hand argument to assert_contains_part does not contain the right hand:\n\n",
    );
  }
}

#[derive(PartialEq, Eq)]
#[doc(hidden)]
pub struct PrettyString<'a>(pub &'a str);

/// Make diff to display string as multi-line string
impl<'a> fmt::Debug for PrettyString<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.write_str(self.0)
  }
}
