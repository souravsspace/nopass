//! The keys each kind of record claims, and the views that read them.
//!
//! Spelling is the user's business, not nopass's: someone who wrote `email:`
//! where this file expected `username:` meant the same thing, and a store
//! full of hand-written entries predates every one of these lists. So each
//! field names its aliases, first occurrence in the body wins, and anything
//! unrecognised is left where it was.
//!
//! Only the reading lives here. What a field *means* to a login form is the
//! extension's business, and what it means on disk is [`crate::record`]'s.

use crate::record::{Kind, Record};

pub const USERNAME_KEYS: &[&str] = &["username", "user", "login", "email"];
pub const URL_KEYS: &[&str] = &["url", "website", "site"];
pub const TOTP_KEYS: &[&str] = &["totp", "otp", "otpauth", "otp_secret"];

pub const CARDHOLDER_KEYS: &[&str] = &["cardholder", "name", "holder"];
pub const CARD_BRAND_KEYS: &[&str] = &["brand", "network", "issuer"];
pub const CARD_EXP_KEYS: &[&str] = &["exp", "expiry", "expires", "expiration"];
pub const CARD_EXP_MONTH_KEYS: &[&str] = &["exp-month", "exp_month", "month"];
pub const CARD_EXP_YEAR_KEYS: &[&str] = &["exp-year", "exp_year", "year"];
pub const CARD_CVV_KEYS: &[&str] = &["cvv", "cvc", "csc", "security-code"];
pub const CARD_ZIP_KEYS: &[&str] = &["zip", "postcode", "postal-code"];

pub const GIVEN_NAME_KEYS: &[&str] = &["given-name", "first-name", "firstname"];
pub const FAMILY_NAME_KEYS: &[&str] = &["family-name", "last-name", "surname", "lastname"];
pub const FULL_NAME_KEYS: &[&str] = &["name", "full-name", "fullname"];
pub const EMAIL_KEYS: &[&str] = &["email", "e-mail", "mail"];
pub const PHONE_KEYS: &[&str] = &["phone", "tel", "telephone", "mobile"];
pub const BIRTHDAY_KEYS: &[&str] = &["birthday", "birthdate", "dob", "born"];
pub const AGE_KEYS: &[&str] = &["age"];
pub const STREET_KEYS: &[&str] = &["street", "address", "street-address", "address-line1"];
pub const STREET2_KEYS: &[&str] = &["address-line2", "street2", "apartment", "unit"];
pub const CITY_KEYS: &[&str] = &["city", "town", "locality"];
pub const REGION_KEYS: &[&str] = &["region", "state", "province", "county"];
pub const POSTCODE_KEYS: &[&str] = &["postcode", "postal-code", "zip", "zipcode"];
pub const COUNTRY_KEYS: &[&str] = &["country", "nation"];
pub const ORGANIZATION_KEYS: &[&str] = &["organization", "organisation", "company", "employer"];

pub const PASSKEY_RP_KEYS: &[&str] = &["rp", "rp-id", "relying-party"];
pub const PASSKEY_USER_KEYS: &[&str] = &["user", "username", "user-name"];
pub const PASSKEY_USER_HANDLE_KEYS: &[&str] = &["user-handle", "user-id"];
pub const PASSKEY_CREDENTIAL_KEYS: &[&str] = &["credential-id", "credential", "id"];
pub const PASSKEY_ALG_KEYS: &[&str] = &["alg", "algorithm"];
pub const PASSKEY_COUNTER_KEYS: &[&str] = &["counter", "sign-count"];

/// One field a kind claims: the name it is written under when nopass writes
/// it, and every spelling it is recognised by when someone else did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldSpec {
    pub key: &'static str,
    pub aliases: &'static [&'static str],
}

const fn spec(key: &'static str, aliases: &'static [&'static str]) -> FieldSpec {
    FieldSpec { key, aliases }
}

const LOGIN_FIELDS: &[FieldSpec] = &[
    spec("username", USERNAME_KEYS),
    spec("url", URL_KEYS),
    spec("totp", TOTP_KEYS),
];

const CARD_FIELDS: &[FieldSpec] = &[
    spec("cardholder", CARDHOLDER_KEYS),
    spec("exp-month", CARD_EXP_MONTH_KEYS),
    spec("exp-year", CARD_EXP_YEAR_KEYS),
    spec("cvv", CARD_CVV_KEYS),
    spec("brand", CARD_BRAND_KEYS),
    spec("zip", CARD_ZIP_KEYS),
];

const IDENTITY_FIELDS: &[FieldSpec] = &[
    spec("given-name", GIVEN_NAME_KEYS),
    spec("family-name", FAMILY_NAME_KEYS),
    spec("email", EMAIL_KEYS),
    spec("phone", PHONE_KEYS),
    spec("birthday", BIRTHDAY_KEYS),
    spec("age", AGE_KEYS),
    spec("street", STREET_KEYS),
    spec("street2", STREET2_KEYS),
    spec("city", CITY_KEYS),
    spec("region", REGION_KEYS),
    spec("postcode", POSTCODE_KEYS),
    spec("country", COUNTRY_KEYS),
    spec("organization", ORGANIZATION_KEYS),
];

const PASSKEY_FIELDS: &[FieldSpec] = &[
    spec("rp", PASSKEY_RP_KEYS),
    spec("user", PASSKEY_USER_KEYS),
    spec("user-handle", PASSKEY_USER_HANDLE_KEYS),
    spec("credential-id", PASSKEY_CREDENTIAL_KEYS),
    spec("alg", PASSKEY_ALG_KEYS),
    spec("counter", PASSKEY_COUNTER_KEYS),
];

/// The fields `kind` claims, in the order a screen should show them.
pub fn spec_for(kind: &Kind) -> &'static [FieldSpec] {
    match kind {
        Kind::Login => LOGIN_FIELDS,
        Kind::Card => CARD_FIELDS,
        Kind::Identity => IDENTITY_FIELDS,
        Kind::Passkey => PASSKEY_FIELDS,
        // A kind this build has never heard of claims nothing, so its lines
        // travel as themselves and come back unchanged.
        Kind::Other(_) => &[],
    }
}

/// Every spelling of a canonical key, whichever kind claims it.
pub fn aliases_for(key: &str) -> Option<&'static [&'static str]> {
    [LOGIN_FIELDS, CARD_FIELDS, IDENTITY_FIELDS, PASSKEY_FIELDS]
        .into_iter()
        .flatten()
        .find(|field| field.key == key)
        .map(|field| field.aliases)
}

/// A login: what the browser fills into a sign-in form.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Login {
    pub password: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub totp: Option<String>,
}

/// A payment card. The number is the record's first line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Card {
    pub number: String,
    pub cardholder: Option<String>,
    /// Two digits, `01`–`12`, whether it was written on its own line or as
    /// half of an `exp:`.
    pub exp_month: Option<String>,
    /// Four digits. `29` is read as `2029`, because that is what is printed
    /// on the card.
    pub exp_year: Option<String>,
    pub cvv: Option<String>,
    pub brand: Option<String>,
    pub zip: Option<String>,
}

/// A person, for the forms that ask who you are.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Identity {
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub birthday: Option<String>,
    pub age: Option<String>,
    pub street: Option<String>,
    pub street2: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postcode: Option<String>,
    pub country: Option<String>,
    pub organization: Option<String>,
}

fn owned(record: &Record, keys: &[&str]) -> Option<String> {
    record.get(keys).map(str::to_string)
}

pub fn login(record: &Record) -> Login {
    Login {
        password: record.primary.clone(),
        username: owned(record, USERNAME_KEYS),
        url: owned(record, URL_KEYS),
        totp: owned(record, TOTP_KEYS).or_else(|| record.raw_starting_with("otpauth://")),
    }
}

pub fn card(record: &Record) -> Card {
    let (month, year) = expiry(record);
    Card {
        number: record.primary.clone(),
        cardholder: owned(record, CARDHOLDER_KEYS),
        exp_month: month,
        exp_year: year,
        cvv: owned(record, CARD_CVV_KEYS),
        brand: owned(record, CARD_BRAND_KEYS),
        zip: owned(record, CARD_ZIP_KEYS),
    }
}

pub fn identity(record: &Record) -> Identity {
    let (given, family) = names(record);
    Identity {
        given_name: given,
        family_name: family,
        email: owned(record, EMAIL_KEYS),
        phone: owned(record, PHONE_KEYS),
        birthday: owned(record, BIRTHDAY_KEYS),
        age: owned(record, AGE_KEYS),
        street: owned(record, STREET_KEYS),
        street2: owned(record, STREET2_KEYS),
        city: owned(record, CITY_KEYS),
        region: owned(record, REGION_KEYS),
        postcode: owned(record, POSTCODE_KEYS),
        country: owned(record, COUNTRY_KEYS),
        organization: owned(record, ORGANIZATION_KEYS),
    }
}

/// Split `exp: 04/2029` into its halves, or read the halves if they were
/// written separately. A card prints `04/29`, so a two-digit year is this
/// century — a card that expired in 1929 is not the case to serve.
fn expiry(record: &Record) -> (Option<String>, Option<String>) {
    let month = owned(record, CARD_EXP_MONTH_KEYS);
    let year = owned(record, CARD_EXP_YEAR_KEYS);
    if month.is_some() || year.is_some() {
        return (month.map(pad_month), year.map(widen_year));
    }

    let Some(combined) = record.get(CARD_EXP_KEYS) else {
        return (None, None);
    };
    let Some((month, year)) = combined.split_once(['/', '-']) else {
        return (None, None);
    };
    (
        Some(pad_month(month.trim().to_string())),
        Some(widen_year(year.trim().to_string())),
    )
}

fn pad_month(month: String) -> String {
    match month.trim().parse::<u32>() {
        Ok(number) if (1..=12).contains(&number) => format!("{number:02}"),
        _ => month,
    }
}

fn widen_year(year: String) -> String {
    let year = year.trim();
    match (year.len(), year.parse::<u32>()) {
        (2, Ok(number)) => format!("20{number:02}"),
        _ => year.to_string(),
    }
}

/// A record may carry the two halves of a name, or one line with both. A
/// single-word name is a given name; the last word of a longer one is the
/// family name, which is wrong for some people and is why both halves can be
/// written explicitly.
fn names(record: &Record) -> (Option<String>, Option<String>) {
    let given = owned(record, GIVEN_NAME_KEYS);
    let family = owned(record, FAMILY_NAME_KEYS);
    if given.is_some() || family.is_some() {
        return (given, family);
    }

    let Some(full) = record.get(FULL_NAME_KEYS) else {
        return (None, None);
    };
    match full.rsplit_once(' ') {
        Some((first, last)) => (
            Some(first.trim().to_string()),
            Some(last.trim().to_string()),
        ),
        None => (Some(full.to_string()), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{Kind, Record};

    fn parsed(body: &str) -> Record {
        Record::parse(body)
    }

    #[test]
    fn a_login_reads_the_keys_it_always_did() {
        let record = parsed("hunter2\nusername: sana\nurl: https://a.b\ntotp: ABC\n");
        assert_eq!(
            login(&record),
            Login {
                password: "hunter2".to_string(),
                username: Some("sana".to_string()),
                url: Some("https://a.b".to_string()),
                totp: Some("ABC".to_string()),
            }
        );
    }

    #[test]
    fn a_login_accepts_the_spellings_people_actually_use() {
        assert_eq!(
            login(&parsed(
                "pw\nemail: sana@example.com\nwebsite: https://a.b\n"
            ))
            .username,
            Some("sana@example.com".to_string())
        );
        assert_eq!(
            login(&parsed("pw\nsite: https://a.b\n")).url,
            Some("https://a.b".to_string())
        );
    }

    #[test]
    fn a_bare_otpauth_line_is_still_the_totp() {
        assert_eq!(
            login(&parsed("pw\notpauth://totp/x?secret=ABC\n")).totp,
            Some("otpauth://totp/x?secret=ABC".to_string())
        );
    }

    #[test]
    fn a_card_number_is_the_first_line() {
        let card = card(&parsed(
            "4111111111111111\ntype: card\ncardholder: Sana Q\n",
        ));
        assert_eq!(card.number, "4111111111111111");
        assert_eq!(card.cardholder.as_deref(), Some("Sana Q"));
    }

    #[test]
    fn a_combined_expiry_splits_into_month_and_year() {
        let card = card(&parsed("4111\ntype: card\nexp: 04/2029\n"));
        assert_eq!(card.exp_month.as_deref(), Some("04"));
        assert_eq!(card.exp_year.as_deref(), Some("2029"));
    }

    #[test]
    fn an_expiry_written_the_way_it_is_printed_on_the_card_is_understood() {
        let card = card(&parsed("4111\ntype: card\nexpiry: 4/29\n"));
        assert_eq!(card.exp_month.as_deref(), Some("04"));
        assert_eq!(card.exp_year.as_deref(), Some("2029"));
    }

    #[test]
    fn separate_expiry_lines_win_over_a_combined_one() {
        let card = card(&parsed(
            "4111\ntype: card\nexp: 01/2020\nexp-month: 12\nexp-year: 2030\n",
        ));
        assert_eq!(card.exp_month.as_deref(), Some("12"));
        assert_eq!(card.exp_year.as_deref(), Some("2030"));
    }

    #[test]
    fn an_expiry_nobody_wrote_is_absent_rather_than_invented() {
        let card = card(&parsed("4111\ntype: card\n"));
        assert_eq!(card.exp_month, None);
        assert_eq!(card.exp_year, None);
    }

    #[test]
    fn a_card_keeps_its_security_code_only_if_one_was_written() {
        assert_eq!(card(&parsed("4111\ntype: card\n")).cvv, None);
        assert_eq!(
            card(&parsed("4111\ntype: card\ncvc: 737\n")).cvv.as_deref(),
            Some("737")
        );
    }

    #[test]
    fn an_identity_reads_a_full_address() {
        let record = parsed(
            "\ntype: identity\ngiven-name: Sana\nfamily-name: Qureshi\nemail: sana@example.com\nphone: +880123\nstreet: 12 Example Road\ncity: Dhaka\npostcode: 1207\ncountry: Bangladesh\n",
        );
        let identity = identity(&record);
        assert_eq!(identity.given_name.as_deref(), Some("Sana"));
        assert_eq!(identity.family_name.as_deref(), Some("Qureshi"));
        assert_eq!(identity.email.as_deref(), Some("sana@example.com"));
        assert_eq!(identity.city.as_deref(), Some("Dhaka"));
        assert_eq!(identity.country.as_deref(), Some("Bangladesh"));
    }

    #[test]
    fn one_name_line_is_split_into_its_halves() {
        let identity = identity(&parsed("\ntype: identity\nname: Sana Qureshi\n"));
        assert_eq!(identity.given_name.as_deref(), Some("Sana"));
        assert_eq!(identity.family_name.as_deref(), Some("Qureshi"));
    }

    #[test]
    fn a_single_word_name_is_a_given_name_and_invents_no_surname() {
        let identity = identity(&parsed("\ntype: identity\nname: Sana\n"));
        assert_eq!(identity.given_name.as_deref(), Some("Sana"));
        assert_eq!(identity.family_name, None);
    }

    #[test]
    fn halves_written_out_beat_a_full_name_line() {
        let identity = identity(&parsed(
            "\ntype: identity\nname: Wrong Person\ngiven-name: Sana\nfamily-name: Qureshi\n",
        ));
        assert_eq!(identity.given_name.as_deref(), Some("Sana"));
        assert_eq!(identity.family_name.as_deref(), Some("Qureshi"));
    }

    #[test]
    fn an_identity_may_hold_an_age_as_well_as_a_birthday() {
        let identity = identity(&parsed("\ntype: identity\ndob: 1996-04-02\nage: 30\n"));
        assert_eq!(identity.birthday.as_deref(), Some("1996-04-02"));
        assert_eq!(identity.age.as_deref(), Some("30"));
    }

    #[test]
    fn every_kind_names_the_fields_it_claims() {
        let login: Vec<&str> = spec_for(&Kind::Login).iter().map(|f| f.key).collect();
        assert_eq!(login, vec!["username", "url", "totp"]);

        let card: Vec<&str> = spec_for(&Kind::Card).iter().map(|f| f.key).collect();
        assert!(card.contains(&"cardholder"), "{card:?}");
        assert!(card.contains(&"exp-month"), "{card:?}");
        assert!(card.contains(&"cvv"), "{card:?}");

        let identity: Vec<&str> = spec_for(&Kind::Identity).iter().map(|f| f.key).collect();
        assert!(identity.contains(&"given-name"), "{identity:?}");
        assert!(identity.contains(&"country"), "{identity:?}");
    }

    #[test]
    fn a_canonical_key_is_the_first_of_its_own_aliases() {
        // Otherwise writing a field into an entry that has none of its
        // spellings would write a key that reading it back would not find.
        for kind in [Kind::Login, Kind::Card, Kind::Identity, Kind::Passkey] {
            for field in spec_for(&kind) {
                assert!(
                    field.aliases.contains(&field.key),
                    "{} is not among its own aliases",
                    field.key
                );
            }
        }
    }

    #[test]
    fn a_canonical_key_is_a_bare_token_the_wire_will_accept() {
        for kind in [Kind::Login, Kind::Card, Kind::Identity, Kind::Passkey] {
            for field in spec_for(&kind) {
                // The wire's rule: a lower-case letter, then letters, digits
                // and hyphens. `street2` is why the digits are allowed.
                let mut chars = field.key.chars();
                assert!(
                    chars.next().is_some_and(|c| c.is_ascii_lowercase())
                        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                    "{} is not a bare token",
                    field.key
                );
                assert_ne!(field.key, "type", "the kind is not a field");
            }
        }
    }

    #[test]
    fn a_key_can_be_looked_up_to_find_the_spellings_it_stands_for() {
        assert_eq!(
            aliases_for("username"),
            Some(&["username", "user", "login", "email"][..])
        );
        assert_eq!(aliases_for("notes"), None);
    }

    #[test]
    fn a_kind_does_not_have_to_be_read_as_the_kind_it_claims() {
        // Reading a card as a login is nonsense, but it must not panic: the
        // wire carries a name and a kind the caller chose to trust.
        let record = parsed("4111\ntype: card\ncardholder: Sana Q\n");
        assert_eq!(record.kind(), Kind::Card);
        assert_eq!(login(&record).password, "4111");
    }
}
