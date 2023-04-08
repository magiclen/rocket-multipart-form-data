#[derive(Debug, Clone, Copy)]
pub(crate) enum RepetitionCounter {
    Fixed(u32),
    Infinite,
}

impl RepetitionCounter {
    #[inline]
    pub fn decrease_check_is_over(&mut self) -> bool {
        match self {
            RepetitionCounter::Fixed(n) => {
                debug_assert!(*n > 0);

                *n -= 1;
                *n == 0
            },
            RepetitionCounter::Infinite => false,
        }
    }
}

impl Default for RepetitionCounter {
    #[inline]
    fn default() -> Self {
        RepetitionCounter::Fixed(1)
    }
}

#[derive(Debug, Clone, Copy)]
/// It can be used to define a `MultipartFormDataField` instance which can be used how many times.
pub struct Repetition {
    counter: RepetitionCounter,
}

impl Repetition {
    #[inline]
    /// Create a `Repetition` instance for only one time.
    pub fn new() -> Repetition {
        Repetition {
            counter: RepetitionCounter::Fixed(1)
        }
    }

    #[inline]
    /// Create a `Repetition` instance for any fixed times.
    pub fn fixed(count: u32) -> Repetition {
        if count == 0 {
            eprintln!(
                "The count of fixed repetition for a `MultipartFormDataField` instance should be \
                 bigger than 0. Use 1 instead."
            );

            Repetition::new()
        } else {
            Repetition {
                counter: RepetitionCounter::Fixed(count)
            }
        }
    }

    #[inline]
    /// Create a `Repetition` instance for infinite times.
    pub fn infinite() -> Repetition {
        Repetition {
            counter: RepetitionCounter::Infinite
        }
    }

    #[inline]
    pub(crate) fn decrease_check_is_over(&mut self) -> bool {
        self.counter.decrease_check_is_over()
    }
}

impl Default for Repetition {
    #[inline]
    /// Create a `Repetition` instance for only one time.
    fn default() -> Self {
        Repetition::new()
    }
}
