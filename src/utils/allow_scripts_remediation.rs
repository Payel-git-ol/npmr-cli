//! Port of `lib/utils/allow-scripts-remediation.js`: the `npm config set`
//! command suggested to global users, who have no project package.json for
//! `npm approve-scripts` to write to.

pub fn config_set_allow_scripts(names: &[String]) -> String {
    format!(
        "npm config set allow-scripts={} --location=user",
        names.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_names_with_location_user() {
        assert_eq!(
            config_set_allow_scripts(&["a".into(), "b".into()]),
            "npm config set allow-scripts=a,b --location=user"
        );
    }
}
