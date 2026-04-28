use crate::{
  selection::Selection,
  transaction::Transaction,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
  pub transaction: Transaction,
  pub selection:   Selection,
}
