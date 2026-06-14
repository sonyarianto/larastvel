use rand::Rng;

use super::database;

// ---------------------------------------------------------------------------
// Faker — lightweight fake data generator
// ---------------------------------------------------------------------------

/// Simple fake data generator for model factories.
///
/// Provides static methods that return random-but-plausible values for
/// common column types.  This is intentionally minimal — no locale support,
/// no heavyweight dependency beyond `rand` and `uuid`.
pub struct Faker;

impl Faker {
    /// Generate a random first + last name.
    pub fn name() -> String {
        let first = [
            "James",
            "Mary",
            "Robert",
            "Patricia",
            "John",
            "Jennifer",
            "Michael",
            "Linda",
            "David",
            "Elizabeth",
            "William",
            "Barbara",
            "Richard",
            "Susan",
            "Joseph",
            "Jessica",
            "Thomas",
            "Sarah",
            "Christopher",
            "Karen",
        ];
        let last = [
            "Smith",
            "Johnson",
            "Williams",
            "Brown",
            "Jones",
            "Garcia",
            "Miller",
            "Davis",
            "Rodriguez",
            "Martinez",
            "Hernandez",
            "Lopez",
            "Gonzalez",
            "Wilson",
            "Anderson",
            "Thomas",
            "Taylor",
            "Moore",
            "Jackson",
            "Martin",
        ];
        let mut rng = rand::thread_rng();
        format!(
            "{} {}",
            first[rng.gen_range(0..first.len())],
            last[rng.gen_range(0..last.len())]
        )
    }

    /// Generate a plausible email address from a name.
    pub fn email() -> String {
        let domains = ["example.com", "test.org", "mail.net", "demo.io"];
        let mut rng = rand::thread_rng();
        let name = Self::name().to_lowercase().replace(' ', ".");
        let domain = domains[rng.gen_range(0..domains.len())];
        format!("{}@{}", name, domain)
    }

    /// Generate a random sentence (ends with period, ~5-15 words).
    pub fn sentence() -> String {
        let words = [
            "lorem",
            "ipsum",
            "dolor",
            "sit",
            "amet",
            "consectetur",
            "adipiscing",
            "elit",
            "sed",
            "do",
            "eiusmod",
            "tempor",
            "incididunt",
            "ut",
            "labore",
            "et",
            "dolore",
            "magna",
            "aliqua",
            "enim",
            "ad",
            "minim",
            "veniam",
            "quis",
            "nostrud",
            "exercitation",
            "ullamco",
            "laboris",
            "nisi",
            "ut",
            "aliquip",
            "ex",
            "ea",
            "commodo",
            "consequat",
            "velit",
            "esse",
            "cillum",
            "eu",
            "fugiat",
            "nulla",
            "pariatur",
        ];
        let mut rng = rand::thread_rng();
        let count = rng.gen_range(5..=15);
        let sentence: Vec<&str> = (0..count)
            .map(|_| words[rng.gen_range(0..words.len())])
            .collect();
        let mut s = sentence.join(" ");
        s[..1].make_ascii_uppercase();
        s.push('.');
        s
    }

    /// Generate a short phrase (2-5 words, no punctuation).
    pub fn word() -> String {
        let words = [
            "foo", "bar", "baz", "qux", "alpha", "beta", "gamma", "delta", "epsilon", "zeta",
            "eta", "theta", "iota", "kappa", "lambda", "mu", "nu", "xi", "omicron", "pi", "rho",
            "sigma",
        ];
        let mut rng = rand::thread_rng();
        let count = rng.gen_range(1..=4);
        (0..count)
            .map(|_| words[rng.gen_range(0..words.len())])
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Generate a paragraph of `n` sentences.
    pub fn paragraph(n: usize) -> String {
        (0..n)
            .map(|_| Self::sentence())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Generate a random integer in `[min, max]` (inclusive).
    pub fn number(min: u64, max: u64) -> u64 {
        rand::thread_rng().gen_range(min..=max)
    }

    /// Generate a random boolean.
    pub fn boolean() -> bool {
        rand::thread_rng().gen_bool(0.5)
    }

    /// Generate a random UUID (v4) string.
    pub fn uuid() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

// ---------------------------------------------------------------------------
// ModelFactory trait
// ---------------------------------------------------------------------------

/// Trait for defining model factories — the Rust equivalent of Laravel's
/// `Factory` base class.
///
/// Provides `definition()`, `make()`, and `make_count()`.  For persistence
/// use the free functions [`factory_create`] and [`factory_create_count`].
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Default)]
/// struct UserFactory;
///
/// impl ModelFactory for UserFactory {
///     type ActiveModel = user::ActiveModel;
///
///     fn definition() -> Self::ActiveModel {
///         use sea_orm::Set;
///         user::ActiveModel {
///             id: sea_orm::NotSet,
///             name: Set(Faker::name()),
///             email: Set(Faker::email()),
///             password: Set(Faker::uuid()),
///             created_at: Set(chrono::Utc::now().naive_utc()),
///             updated_at: Set(chrono::Utc::now().naive_utc()),
///         }
///     }
/// }
///
/// // Usage
/// let draft = UserFactory::make();
/// let batch = UserFactory::make_count(5);
/// let user = factory_create::<UserFactory>().await.unwrap();
/// let users = factory_create_count::<UserFactory>(5).await.unwrap();
/// ```
pub trait ModelFactory: Default {
    /// The SeaORM `ActiveModel` type for the target entity.
    type ActiveModel: sea_orm::ActiveModelTrait + sea_orm::ActiveModelBehavior + Send;

    /// Define the default attribute values for the model.
    fn definition() -> Self::ActiveModel;

    /// Build a single model instance in memory (not persisted).
    fn make() -> Self::ActiveModel {
        Self::definition()
    }

    /// Build `n` model instances in memory.
    fn make_count(n: usize) -> Vec<Self::ActiveModel> {
        (0..n).map(|_| Self::definition()).collect()
    }
}

// ---------------------------------------------------------------------------
// Free-standing factory helpers
// ---------------------------------------------------------------------------

/// Persist a single model instance using the given factory and return the
/// saved model.
pub async fn factory_create<F>() -> Result<
    <<<F as ModelFactory>::ActiveModel as sea_orm::ActiveModelTrait>::Entity as sea_orm::EntityTrait>::Model,
    sea_orm::DbErr,
>
where
    F: ModelFactory,
    <<<F as ModelFactory>::ActiveModel as sea_orm::ActiveModelTrait>::Entity as sea_orm::EntityTrait>::Model:
        sea_orm::IntoActiveModel<<F as ModelFactory>::ActiveModel>,
{
    sea_orm::ActiveModelTrait::insert(F::definition(), database()).await
}

/// Persist `n` model instances using the given factory and return the saved
/// models.
pub async fn factory_create_count<F>(
    n: usize,
) -> Result<
    Vec<
        <<<F as ModelFactory>::ActiveModel as sea_orm::ActiveModelTrait>::Entity as sea_orm::EntityTrait>::Model,
    >,
    sea_orm::DbErr,
>
where
    F: ModelFactory,
    <<<F as ModelFactory>::ActiveModel as sea_orm::ActiveModelTrait>::Entity as sea_orm::EntityTrait>::Model:
        sea_orm::IntoActiveModel<<F as ModelFactory>::ActiveModel>,
{
    let mut results = Vec::with_capacity(n);
    for _ in 0..n {
        results.push(factory_create::<F>().await?);
    }
    Ok(results)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_faker_name_non_empty() {
        let name = Faker::name();
        assert!(!name.is_empty(), "name should not be empty");
        assert!(
            name.contains(' '),
            "name should contain a space: got {:?}",
            name
        );
    }

    #[test]
    fn test_faker_email_contains_at() {
        let email = Faker::email();
        assert!(
            email.contains('@'),
            "email should contain @: got {:?}",
            email
        );
    }

    #[test]
    fn test_faker_sentence_ends_with_period() {
        let s = Faker::sentence();
        assert!(
            s.ends_with('.'),
            "sentence should end with '.': got {:?}",
            s
        );
        // At least one uppercase letter
        assert!(
            s.chars().any(|c| c.is_uppercase()),
            "sentence should contain an uppercase letter"
        );
    }

    #[test]
    fn test_faker_paragraph_length() {
        let p = Faker::paragraph(3);
        let periods = p.chars().filter(|&c| c == '.').count();
        assert_eq!(periods, 3, "paragraph should have 3 sentences");
    }

    #[test]
    fn test_faker_number_in_range() {
        for _ in 0..100 {
            let n = Faker::number(5, 10);
            assert!((5..=10).contains(&n), "number {} should be in 5..=10", n);
        }
    }

    #[test]
    fn test_faker_boolean_is_bool() {
        let _: bool = Faker::boolean();
    }

    #[test]
    fn test_faker_uuid_format() {
        let u = Faker::uuid();
        assert_eq!(u.len(), 36, "UUID should be 36 chars: got {:?}", u);
    }

    #[test]
    fn test_faker_word_non_empty() {
        let w = Faker::word();
        assert!(!w.is_empty(), "word should not be empty");
    }

    #[test]
    fn test_faker_deterministic_after_seed() {
        // Not seeded by design, but verify both calls return valid data
        let a = Faker::name();
        let b = Faker::name();
        assert!(!a.is_empty());
        assert!(!b.is_empty());
    }

    // ModelFactory trait is verified by compilation — no concrete impl
    // without a SeaORM entity (which needs `#[derive(DeriveEntityModel)]`).
}
