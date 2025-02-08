use std::{
    // Import BTreeMap and HashMap from the standard library.
    // BTreeMap is used for tree expressions, which are ordered maps.
    // HashMap is used for map expressions, which are unordered maps,
    // and for the symbol table plus environment bindings.
    collections::HashMap,
    // Import the necessary types and traits for formatting our output.
    fmt::{Debug, Display, Formatter, Result as FmtResult},
    // Import hash types and traits for hashing our expressions,
    // to allow them to be used as keys in a hash map.
    hash::Hash,
    // Import atomic reference counting for shared ownership of symbols,
    // and read-write locks for the symbol table.
    sync::{Arc, RwLock},
};

// Use lazy_static for setting up the symbol table as a global variable.
use lazy_static::lazy_static;

///////////////////////////////////////////////////////////////
// SYMBOLS AND SYMBOL TABLE
///////////////////////////////////////////////////////////////

/*
 * The symbol table is a hash map that maps strings to symbols.
 * It uses string interning to ensure that symbols are unique,
 * and to allow for fast comparison of symbols.
 */

 lazy_static! {
    /// The symbol table that maps strings to symbols
    /// 
    /// This is a global variable that is shared between all environments.
    /// It is a read-write lock that allows for multiple environments to
    /// read from the symbol table at the same time, but only one environment
    /// to write to the symbol table at a time.
    static ref SYMBOLS: RwLock<HashMap<String, Symbol>> = RwLock::new(HashMap::new());
}

/// A symbol that uses string interning
#[derive(Clone, Hash, Eq, Ord)]
pub struct Symbol(Arc<String>);

impl Symbol {
    /// Create a new symbol from a string
    /// 
    /// If the symbol already exists in the symbol table, it will return the existing symbol.
    /// Otherwise, it will create a new symbol and add it to the symbol table.
    pub fn new(name: &str) -> Self {
        // Check if the symbol already exists
        let mut symbols = SYMBOLS.write().unwrap();
        // If the symbol already exists, return it
        if let Some(symbol) = symbols.get(name) {
            return symbol.clone();
        }

        // Otherwise, create a new symbol
        let symbol = Symbol(Arc::new(name.to_string()));
        // Add the symbol to the symbol table
        symbols.insert(name.to_string(), symbol.clone());
        symbol
    }

    /// Get the name of the symbol as a string
    /// 
    /// This is useful when you need the internal string representation of the symbol.
    pub fn name(&self) -> &str {
        &self.0
    }
}

/// Convert a &str to a symbol conveniently
/// 
/// This allows you to pass a string to a function that expects a symbol,
/// using the `into()` method.
impl From<&str> for Symbol {
    #[inline]
    fn from(s: &str) -> Self {
        Symbol::new(s)
    }
}

/// Convert a String to a symbol conveniently
/// 
/// This allows you to pass a string to a function that expects a symbol,
/// using the `into()` method.
impl From<String> for Symbol {
    #[inline]
    fn from(s: String) -> Self {
        Symbol::new(&s)
    }
}

/// Compare two symbols for equality
/// 
/// This allows you to compare two symbols using the `==` operator.
/// First, it checks if the two symbols are the same object in memory.
/// If they are not, it compares the internal strings of the symbols.
/// 
/// This is faster than comparing the strings directly, because a pointer comparison
/// is faster than a string comparison.
impl PartialEq for Symbol {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Check if the two symbols are the same object in memory
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        // Compare the internal strings of the symbols
        self.0 == other.0
    }
}

/// Compare two symbols for ordering.
/// 
/// If the two symbols are the same object in memory, they are equal.
/// Otherwise, it compares the internal strings of the symbols.
impl PartialOrd for Symbol {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if Arc::ptr_eq(&self.0, &other.0) {
            return Some(std::cmp::Ordering::Equal);
        }
        self.0.partial_cmp(&other.0)
    }
}

/// Print a symbol as standard output
impl Display for Symbol {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

/// Print a symbol as debug output
/// 
/// Since a symbol is meant to be an identifier, it is printed as a normal string.
/// This is useful for debugging, because it allows you to distinguish symbols from strings.
impl Debug for Symbol {
    #[inline]
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", self.0)
    }
}

/// Convert a symbol to a string reference
/// 
/// This allows you to pass a symbol to
/// a function that expects a string reference,
/// using the `as_ref()` method.
impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}