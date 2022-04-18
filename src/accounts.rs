// std
use std::collections::HashMap;
use std::io::Write;
use std::{error, fmt};
// third party libs
use serde::Serialize;
// internal
use crate::csv_reader::{Transaction, TransactionKind};

/// An account for a client
#[derive(Debug)]
struct Account {
    available: f64,
    held: f64,
    total: f64,
    locked: bool,
}

impl Account {
    fn new() -> Self {
        Self {
            available: 0.0,
            held: 0.0,
            total: 0.0,
            locked: false,
        }
    }
}

/// An error retrieved via [Accounts::handle_transaction]
#[derive(Debug, Clone)]
pub enum TransactionError {
    /// Client is unknown (this should never happen)
    UnknownClient(u16),
    /// Transaction is unknown (e.g. a Dispute with an unknown tx)
    UnknownTransaction(u32),
    /// Invalid amount (e.g. a deposit with infinite amount)
    InvalidAmount(f64),
    /// Account has reached the f64 limits (should never happen?)
    AccountAmountTooLarge,
    /// Reject a resolve / chargeback transaction because it is not disputed
    TxNonDisputed(u32),
    /// Account is locked thus cannot deposit nor withdraw
    AccountLocked(u16),
    /// Invalid transaction (e.g. non unique tx?)
    InvalidTransaction(u32),
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TransactionError::UnknownClient(c) => {
                write!(f, "Unknown client (client id: {})", c)
            }
            TransactionError::UnknownTransaction(tx) => {
                write!(f, "Unknown transaction (tx: {})", tx)
            }
            TransactionError::InvalidAmount(a) => {
                write!(f, "Invalid amount: {}", a)
            }
            TransactionError::AccountAmountTooLarge => {
                write!(f, "Account amount is too large")
            }
            TransactionError::TxNonDisputed(tx) => {
                write!(f, "Transaction {} is not disputed", tx)
            }
            TransactionError::AccountLocked(c) => {
                write!(f, "Account (client id: {}) is locked", c)
            }
            TransactionError::InvalidTransaction(tx) => {
                write!(f, "Invalid or non unique transaction (tx: {})", tx)
            }
        }
    }
}

impl error::Error for TransactionError {}

/// An opaque data holding all accounts information
pub struct Accounts {
    inner: HashMap<u16, Account>,  // k: client id, v: Account data
    tx: HashMap<u32, Transaction>, // k: tx (aka transaction IDs), v: Transaction struct
}

impl Accounts {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            tx: HashMap::new(),
        }
    }

    #[doc(hidden)]
    fn add_client(&mut self, client_id: u16) {

        self.inner
            .entry(client_id)
            .or_insert_with(|| Account::new());
    }

    #[doc(hidden)]
    fn get_client_account(&self, client_id: u16) -> Option<&Account> {
        self.inner.get(&client_id)
    }

    #[doc(hidden)]
    fn try_get_client_account(&mut self, client_id: u16) -> Result<&mut Account, TransactionError> {
        self.inner
            .get_mut(&client_id)
            .ok_or(TransactionError::UnknownClient(client_id))
    }

    #[doc(hidden)]
    fn get_transaction(&self, tx: u32) -> Option<&Transaction> {
        self.tx.get(&tx)
    }

    #[doc(hidden)]
    fn get_transaction_mut(&mut self, tx: u32) -> Option<&mut Transaction> {
        self.tx.get_mut(&tx)
    }

    /// Generate csv for all accounts (header: client, available, held, total, locked)
    pub fn output_as_csv<W>(&self, into: Option<&mut W>) -> Result<(), csv::Error>
    where
        W: Write,
    {
        #[derive(Debug, Serialize)]
        struct AccountLine {
            client: u16,
            available: f64,
            held: f64,
            total: f64,
            locked: bool,
        }

        impl AccountLine {
            fn from_account(client: u16, account: &Account) -> Self {
                // Create a AccountLine from a client id and its account
                Self {
                    client,
                    available: account.available,
                    held: account.held,
                    total: account.total,
                    locked: account.locked,
                }
            }
        }

        let mut wtr = csv::Writer::from_writer(into.unwrap());

        let res: Result<Vec<()>, csv::Error> = self
            .inner
            .iter()
            .map(|(client, a)| wtr.serialize(AccountLine::from_account(*client, a)))
            .collect();

        res?;
        wtr.flush()?;
        Ok(())
    }

    /// Handle a transaction, returning a [TransactionError] if it fails
    pub fn handle_transaction(&mut self, transaction: Transaction) -> Result<(), TransactionError> {

        self.inner
            .entry(transaction.client)
            .or_insert_with(|| Account::new());

        let amount = get_amount(&transaction)?;

        match transaction.kind {
            TransactionKind::Deposit => {
                if self.tx.contains_key(&transaction.tx) {
                    return Err(TransactionError::InvalidTransaction(transaction.tx));
                }

                let account = self.try_get_client_account(transaction.client)?;

                if account.locked {
                    return Err(TransactionError::AccountLocked(transaction.client));
                }

                let account_avail = account.available;
                let account_total = account.total;

                account.available += amount;
                account.total += amount;

                if ((account.available == account_avail) || (account.total == account_total))
                    && amount != 0.0
                {
                    return Err(TransactionError::AccountAmountTooLarge);
                }

                // keep track of our transaction
                self.tx.insert(transaction.tx, transaction);
            }
            TransactionKind::Withdrawal => {
                if self.tx.contains_key(&transaction.tx) {
                    return Err(TransactionError::InvalidTransaction(transaction.tx));
                }

                let account = self.try_get_client_account(transaction.client)?;

                if account.locked {
                    return Err(TransactionError::AccountLocked(transaction.client));
                }

                if amount > account.available {
                    return Err(TransactionError::InvalidAmount(amount));
                }
                account.available -= amount;
                account.total -= amount;

                // keep track of our transaction
                self.tx.insert(transaction.tx, transaction);
            }
            TransactionKind::Dispute => {
                let matching_transaction = self
                    .get_transaction(transaction.tx)
                    .ok_or(TransactionError::UnknownTransaction(transaction.tx))?;
                let amount_of_matching_tr = get_amount(matching_transaction)?;

                let account = self.try_get_client_account(transaction.client)?;

                account.available -= amount_of_matching_tr;
                account.held += amount_of_matching_tr;

                // XXX: not a fan of this... :-/
                let matching_transaction = self
                    .get_transaction_mut(transaction.tx)
                    .ok_or(TransactionError::UnknownTransaction(transaction.tx))?;

                matching_transaction.under_dispute = true;
            }
            TransactionKind::Resolve => {
                let matching_transaction = self
                    .get_transaction(transaction.tx)
                    .ok_or(TransactionError::UnknownTransaction(transaction.tx))?;

                if !matching_transaction.under_dispute {
                    return Err(TransactionError::TxNonDisputed(transaction.tx));
                }

                let amount_of_matching_tr = get_amount(matching_transaction)?;

                let account = self.try_get_client_account(transaction.client)?;

                account.held -= amount_of_matching_tr;
                account.available += amount_of_matching_tr;
            }
            TransactionKind::Chargeback => {
                let matching_transaction = self
                    .get_transaction(transaction.tx)
                    .ok_or(TransactionError::UnknownTransaction(transaction.tx))?;
                if !matching_transaction.under_dispute {
                    return Err(TransactionError::TxNonDisputed(transaction.tx));
                }

                let amount_of_matching_tr = get_amount(matching_transaction)?;

                let account = self.try_get_client_account(transaction.client)?;

                account.held -= amount_of_matching_tr;
                account.total -= amount_of_matching_tr;
                account.locked = true;
            }
        }

        Ok(())
    }
}

/// Get amount of money for a given [Transaction], returning 0.0 on None
fn get_amount(transaction: &Transaction) -> Result<f64, TransactionError> {
    match transaction.amount {
        Some(a) => {
            if a > 0.0 {
                Ok(a)
            } else {
                Err(TransactionError::InvalidAmount(a))
            }
        }
        None => Ok(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn accounts_output_ok() -> Result<(), Box<dyn Error>> {
        let mut accounts = Accounts::new();

        accounts.add_client(1);
        accounts.add_client(2);

        let mut output: Vec<u8> = Vec::new();
        accounts.output_as_csv(Some(&mut output))?;

        let output_str = std::str::from_utf8(&output).unwrap();
        // println!("output: {:?}", output_str);

        assert!(
            output_str == "client,available,held,total,locked\n1,0.0,0.0,0.0,false\n2,0.0,0.0,0.0,false\n" ||
            output_str == "client,available,held,total,locked\n2,0.0,0.0,0.0,false\n1,0.0,0.0,0.0,false\n"
        );

        // let mut stdout = std::io::stdout();
        // accounts.output_as_csv(Some(&mut stdout));

        Ok(())
    }

    #[test]
    fn accounts_valid_deposit() -> Result<(), Box<dyn Error>> {
        let mut accounts = Accounts::new();

        let client_id = 1;
        let deposit_amount = 25.11;
        let transaction =
            Transaction::new(TransactionKind::Deposit, client_id, 1, Some(deposit_amount));

        accounts.handle_transaction(transaction)?;

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        assert_eq!(account.available, deposit_amount);
        assert_eq!(account.total, deposit_amount);
        assert_eq!(account.held, 0.0);
        Ok(())
    }

    #[test]
    fn accounts_invalid_deposit() -> Result<(), Box<dyn Error>> {
        // Testing deposit < 0, == NAN, == f64::MAX + 9999
        // Check for distinct error on each cases

        let mut accounts = Accounts::new();

        let client_id = 1;

        let deposit_amount0 = -42.42;
        let transaction0 = Transaction::new(
            TransactionKind::Deposit,
            client_id,
            1,
            Some(deposit_amount0),
        );

        let deposit_amount0_1 = f64::NAN;
        let transaction0_1 = Transaction::new(
            TransactionKind::Deposit,
            client_id,
            2,
            Some(deposit_amount0_1),
        );

        let deposit_amount1 = f64::MAX;
        let transaction1 = Transaction::new(
            TransactionKind::Deposit,
            client_id,
            3,
            Some(deposit_amount1),
        );

        let deposit_amount2 = 9999.0;
        let transaction2 = Transaction::new(
            TransactionKind::Deposit,
            client_id,
            4,
            Some(deposit_amount2),
        );

        match accounts.handle_transaction(transaction0) {
            Err(TransactionError::InvalidAmount(a)) => {
                assert_eq!(a, deposit_amount0);
            }
            _ => {
                panic!("Not an error?");
            }
        }

        match accounts.handle_transaction(transaction0_1) {
            Err(TransactionError::InvalidAmount(a)) => {
                assert!(a.is_nan());
            }
            _ => {
                panic!("Not an error?");
            }
        }

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        assert_eq!(account.available, 0.0);
        assert_eq!(account.total, 0.0);
        assert_eq!(account.held, 0.0);

        accounts.handle_transaction(transaction1)?;

        match accounts.handle_transaction(transaction2) {
            Err(TransactionError::AccountAmountTooLarge) => {}
            _ => {
                panic!("Not an error?");
            }
        }

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        println!("account: {:?}", account);

        assert_eq!(account.available, deposit_amount1);
        assert_eq!(account.total, deposit_amount1);
        assert_eq!(account.held, 0.0);
        assert!(account.available.is_normal());
        assert!(account.total.is_normal());

        Ok(())
    }

    #[test]
    fn accounts_valid_withdrawal() -> Result<(), Box<dyn Error>> {
        let mut accounts = Accounts::new();

        let client_id = 1;
        let deposit_amount = 25.11;
        let transaction =
            Transaction::new(TransactionKind::Deposit, client_id, 1, Some(deposit_amount));

        let withdraw_amount = 25.0;
        let transaction1 = Transaction::new(
            TransactionKind::Withdrawal,
            client_id,
            2,
            Some(withdraw_amount),
        );

        accounts.handle_transaction(transaction)?;
        accounts.handle_transaction(transaction1)?;

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        let left_amount = deposit_amount - withdraw_amount;

        assert_eq!(account.available, left_amount);
        assert_eq!(account.total, left_amount);
        assert_eq!(account.held, 0.0);
        Ok(())
    }

    #[test]
    fn accounts_withdraw_too_much() -> Result<(), Box<dyn Error>> {
        let mut accounts = Accounts::new();

        let client_id = 1;
        let deposit_amount = 25.11;
        let transaction =
            Transaction::new(TransactionKind::Deposit, client_id, 1, Some(deposit_amount));

        let withdraw_amount = 42.0;
        let transaction1 = Transaction::new(
            TransactionKind::Withdrawal,
            client_id,
            2,
            Some(withdraw_amount),
        );

        accounts.handle_transaction(transaction)?;
        match accounts.handle_transaction(transaction1) {
            Err(TransactionError::InvalidAmount(a)) => {
                assert_eq!(a, withdraw_amount);
            }
            _ => {
                panic!("No error??");
            }
        };

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        assert_eq!(account.available, deposit_amount);
        assert_eq!(account.total, deposit_amount);
        assert_eq!(account.held, 0.0);
        Ok(())
    }

    #[test]
    fn accounts_dispute_then_resolve() -> Result<(), Box<dyn Error>> {
        let mut accounts = Accounts::new();

        let client_id = 1;
        let deposit_amount = 25.11;
        let transaction1 =
            Transaction::new(TransactionKind::Deposit, client_id, 1, Some(deposit_amount));

        let transaction2 = Transaction::new(TransactionKind::Dispute, client_id, 1, None);

        accounts.handle_transaction(transaction1)?;
        accounts.handle_transaction(transaction2)?;

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        assert_eq!(account.available, 0.0);
        assert_eq!(account.total, deposit_amount);
        assert_eq!(account.held, deposit_amount);

        // Now Resolve

        let transaction3 = Transaction::new(TransactionKind::Resolve, client_id, 1, None);

        accounts.handle_transaction(transaction3)?;

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        assert_eq!(account.available, deposit_amount);
        assert_eq!(account.total, deposit_amount);
        assert_eq!(account.held, 0.0);

        Ok(())
    }

    #[test]
    fn accounts_dispute_then_chargebacks() -> Result<(), Box<dyn Error>> {
        let mut accounts = Accounts::new();

        let client_id = 1;
        let deposit_amount = 25.11;
        let transaction1 =
            Transaction::new(TransactionKind::Deposit, client_id, 1, Some(deposit_amount));

        let transaction2 = Transaction::new(TransactionKind::Dispute, client_id, 1, None);

        accounts.handle_transaction(transaction1)?;
        accounts.handle_transaction(transaction2)?;

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        assert_eq!(account.available, 0.0);
        assert_eq!(account.total, deposit_amount);
        assert_eq!(account.held, deposit_amount);

        // Now Chargeback

        let transaction3 = Transaction::new(TransactionKind::Chargeback, client_id, 1, None);

        accounts.handle_transaction(transaction3)?;

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        assert_eq!(account.available, 0.0);
        assert_eq!(account.total, 0.0);
        assert_eq!(account.held, 0.0);
        assert_eq!(account.locked, true);

        // Try another Deposit (should be rejected as account is locked)

        let transaction3 =
            Transaction::new(TransactionKind::Deposit, client_id, 2, Some(deposit_amount));

        match accounts.handle_transaction(transaction3) {
            Err(TransactionError::AccountLocked(client_id_)) => {
                assert_eq!(client_id_, client_id);
            }
            _ => {
                panic!("No error??");
            }
        }

        Ok(())
    }

    #[test]
    fn accounts_resolve_non_disputed() -> Result<(), Box<dyn Error>> {
        let mut accounts = Accounts::new();

        let client_id = 1;
        let deposit_amount = 25.11;
        let tx = 1;
        let transaction1 = Transaction::new(
            TransactionKind::Deposit,
            client_id,
            tx,
            Some(deposit_amount),
        );

        accounts.handle_transaction(transaction1)?;

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        assert_eq!(account.available, deposit_amount);
        assert_eq!(account.total, deposit_amount);
        assert_eq!(account.held, 0.0);

        // Now Resolve

        let transaction3 = Transaction::new(TransactionKind::Resolve, client_id, tx, None);

        match accounts.handle_transaction(transaction3) {
            Err(TransactionError::TxNonDisputed(tx_)) => {
                assert_eq!(tx, tx_);
            }
            _ => {
                panic!("No error??")
            }
        };

        let account: &Account = accounts
            .get_client_account(client_id)
            .ok_or("Cannot client client account")?;

        assert_eq!(account.available, deposit_amount);
        assert_eq!(account.total, deposit_amount);
        assert_eq!(account.held, 0.0);

        Ok(())
    }

    #[test]
    fn accounts_non_unique_tx() {
        let mut accounts = Accounts::new();

        let client_id = 1;
        let tx = 1;
        let deposit_amount = 25.11;
        let withdraw_amount = 5.99;
        let transaction1 = Transaction::new(
            TransactionKind::Deposit,
            client_id,
            tx,
            Some(deposit_amount),
        );
        let transaction2 = Transaction::new(
            TransactionKind::Withdrawal,
            client_id,
            tx,
            Some(withdraw_amount),
        );

        accounts.handle_transaction(transaction1).unwrap();

        match accounts.handle_transaction(transaction2) {
            Err(TransactionError::InvalidTransaction(tx_)) => {
                assert_eq!(tx_, tx)
            }
            _ => {
                panic!("No error??")
            }
        };
    }
}
