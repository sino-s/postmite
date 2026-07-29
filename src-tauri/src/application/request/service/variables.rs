#[derive(Clone)]
struct ScopedVariable {
    source: VariableSource,
    value: VariableValue,
}

struct VariableScope {
    variables: HashMap<String, ScopedVariable>,
}

impl VariableScope {
    fn from_snapshot(snapshot: &RequestWorkspaceSnapshot) -> Self {
        let mut variables = snapshot
            .collection_variables
            .iter()
            .map(|item| {
                (
                    item.variable.name.clone(),
                    ScopedVariable {
                        source: VariableSource::Collection,
                        value: item.variable.value.clone(),
                    },
                )
            })
            .collect::<HashMap<_, _>>();

        let selected_environment_id = snapshot
            .environments
            .iter()
            .find(|environment| environment.is_selected)
            .map(|environment| environment.id);
        if let Some(environment_id) = selected_environment_id {
            for item in snapshot
                .environment_variables
                .iter()
                .filter(|item| item.environment_id == environment_id)
            {
                variables.insert(
                    item.variable.name.clone(),
                    ScopedVariable {
                        source: VariableSource::Environment,
                        value: item.variable.value.clone(),
                    },
                );
            }
        }

        Self { variables }
    }
}

#[derive(Default)]
struct ResolutionState {
    references: HashMap<String, ResolvedVariableReference>,
    errors: HashMap<String, VariableResolutionError>,
}

fn resolve_text(
    input: &str,
    scope: &VariableScope,
    state: &mut ResolutionState,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedValue {
    resolve_text_with_stack(input, scope, state, &mut Vec::new(), secret_resolver)
}

fn resolve_text_with_stack(
    input: &str,
    scope: &VariableScope,
    state: &mut ResolutionState,
    stack: &mut Vec<String>,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedValue {
    let mut output = String::new();
    let mut contains_secret = false;
    let mut cursor = 0;
    while let Some(start) = input[cursor..].find("{{") {
        let absolute_start = cursor + start;
        output.push_str(&input[cursor..absolute_start]);
        let after_start = absolute_start + 2;
        let Some(end) = input[after_start..].find("}}") else {
            output.push_str(&input[absolute_start..]);
            return ResolvedValue {
                value: output,
                contains_secret,
            };
        };
        let absolute_end = after_start + end;
        let name = input[after_start..absolute_end].trim();
        let resolved = resolve_variable(name, scope, state, stack, secret_resolver);
        if resolved.contains_secret {
            contains_secret = true;
        }
        output.push_str(&resolved.value);
        cursor = absolute_end + 2;
    }
    output.push_str(&input[cursor..]);

    ResolvedValue {
        value: output,
        contains_secret,
    }
}

fn resolve_variable(
    name: &str,
    scope: &VariableScope,
    state: &mut ResolutionState,
    stack: &mut Vec<String>,
    secret_resolver: &dyn Fn(&str) -> Option<String>,
) -> ResolvedValue {
    if stack.iter().any(|item| item == name) {
        state.errors.insert(
            format!("{name}:cycle"),
            VariableResolutionError {
                name: name.to_owned(),
                kind: VariableResolutionErrorKind::Cycle,
            },
        );
        return ResolvedValue {
            value: format!("{{{{{name}}}}}"),
            contains_secret: false,
        };
    }

    let Some(variable) = scope.variables.get(name) else {
        state.errors.insert(
            format!("{name}:missing"),
            VariableResolutionError {
                name: name.to_owned(),
                kind: VariableResolutionErrorKind::Missing,
            },
        );
        return ResolvedValue {
            value: format!("{{{{{name}}}}}"),
            contains_secret: false,
        };
    };

    stack.push(name.to_owned());
    let value = match &variable.value {
        VariableValue::Plain(value) => {
            resolve_text_with_stack(value, scope, state, stack, secret_resolver)
        }
        VariableValue::SecretReference(reference) => ResolvedValue {
            value: secret_resolver(reference).unwrap_or_else(|| REDACTED_VALUE.to_owned()),
            contains_secret: true,
        },
    };
    stack.pop();

    state.references.insert(
        name.to_owned(),
        ResolvedVariableReference {
            name: name.to_owned(),
            source: variable.source,
            value: value.clone(),
        },
    );

    value
}

impl RequestError {
    pub fn persistence(error: impl std::error::Error) -> Self {
        Self::Persistence(error.to_string())
    }
}
