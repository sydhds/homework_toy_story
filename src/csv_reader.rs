// std
use std::fs::File;
use std::path::PathBuf;

// third party libs
use csv::{Reader, Trim};
use serde::Deserialize;

/// Transaction type that we can handle
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionKind {
    /// A Deposit of money on an account
    Deposit,
    /// A Withdrawal of money on an account
    Withdrawal,
    /// A Dispute (on a given transaction)
    Dispute,
    /// A Resolve for an already disputed transaction
    Resolve,
    /// A Chargeback for an already disputed transaction
    Chargeback,
}

/// A Transaction that can be applied to an Account
#[derive(Debug, Deserialize)]
pub struct Transaction {
    #[serde(rename(deserialize = "type"))]
    pub kind: TransactionKind,
    /// a client id (assume 1 client = 1 account)
    pub client: u16,
    /// a transaction id (globally unique)
    pub tx: u32,
    /// amount of money
    #[serde(deserialize_with = "csv::invalid_option")]
    pub amount: Option<f64>, // TODO: f32 or f64?
    /// Is this transaction already referenced by a Dispute? (for Resolve & Chargeback)
    #[serde(skip)]
    pub under_dispute: bool,
}

impl Transaction {
    /// Init a Transaction from scratch (only for unit tests)
    /// Use `CsvReader` to get a list of Transaction
    pub fn new(kind: TransactionKind, client: u16, tx: u32, amount: Option<f64>) -> Self {
        Self {
            kind,
            client,
            tx,
            amount,
            under_dispute: false,
        }
    }
}

/// Our csv reader & iterator (over `Transaction`)
pub struct CsvReader {
    // csv_path: PathBuf,
    rdr: Reader<File>,
}

impl CsvReader {
    pub fn new(csv_path: PathBuf) -> Result<Self, std::io::Error> {
        // let mut rdr = csv::Reader::from_reader(csv_path);

        let rdr = csv::ReaderBuilder::new()
            .trim(Trim::All)
            .has_headers(true)
            .from_path(csv_path)?;

        // println!("rdr: {:?}", rdr);

        /*
        for result in rdr.deserialize() {
            // Notice that we need to provide a type hint for automatic
            // deserialization.

            let tr: Transaction = result.unwrap();
            println!("tr: {:?}", tr);
            // ops.push(op);
        }
        */

        Ok(CsvReader {
            // csv_path,
            rdr,
        })
    }
}

impl Iterator for CsvReader {
    type Item = Result<Transaction, csv::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        for result in self.rdr.deserialize() {
            return Some(result);
        }

        None
        /*
        Some(Transaction {
            kind: TransactionKind::deposit,
            client: 0,
            tx: 0,
            amount: None,
            under_dispute: false
        })
        */

        /*
        loop {
            let i = self.curr;
            self.curr += 1;
            if i % 2 == 0 {
                return Some(i);
            }
        }
        */
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    #[should_panic]
    fn csv_read_non_existing() {
        let csv_1 = PathBuf::from("resources/aa.csv");
        let _csv_reader = CsvReader::new(csv_1).unwrap();
    }

    #[test]
    fn csv_read_valid_sample() -> Result<(), std::io::Error> {
        let csv_1 = PathBuf::from("resources/sample_1.csv");
        let csv_reader = CsvReader::new(csv_1)?;
        let transactions: Result<Vec<Transaction>, _> = csv_reader.map(|t| t).collect();

        assert!(transactions.is_ok());
        assert_eq!(transactions.unwrap().len(), 5);
        Ok(())
    }

    #[test]
    fn csv_read_with_errors() -> Result<(), std::io::Error> {
        let csv_1 = PathBuf::from("resources/sample_1_with_errors.csv");
        let csv_reader = CsvReader::new(csv_1)?;
        let transactions: Result<Vec<Transaction>, _> = csv_reader.map(|t| t).collect();
        assert!(transactions.is_err());
        Ok(())
    }

    #[test]
    fn csv_read_valid_with_spaces() -> Result<(), std::io::Error> {
        let csv_2 = PathBuf::from("resources/sample_2.csv");
        let csv_reader = CsvReader::new(csv_2)?;
        let transactions: Result<Vec<Transaction>, _> = csv_reader.map(|t| t).collect();

        assert!(transactions.is_ok());
        assert_eq!(transactions.unwrap().len(), 5);
        Ok(())
    }
}
