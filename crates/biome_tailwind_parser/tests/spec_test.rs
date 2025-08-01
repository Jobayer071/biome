use biome_console::fmt::{Formatter, Termcolor};
use biome_console::markup;
use biome_diagnostics::{DiagnosticExt, PrintDiagnostic, termcolor};
use biome_rowan::{AstNode, SyntaxKind};
use biome_tailwind_parser::parse_tailwind;
use biome_tailwind_syntax::TwRoot;
use biome_test_utils::{has_bogus_nodes_or_empty_slots, validate_eof_token};
use camino::Utf8Path;
use std::fmt::Write;
use std::fs;

#[derive(Copy, Clone)]
pub enum ExpectedOutcome {
    Pass,
    Fail,
    Undefined,
}

pub fn run(test_case: &str, _snapshot_name: &str, test_directory: &str, outcome_str: &str) {
    let outcome = match outcome_str {
        "ok" => ExpectedOutcome::Pass,
        "error" => ExpectedOutcome::Fail,
        "undefined" => ExpectedOutcome::Undefined,
        _ => panic!("Invalid expected outcome {outcome_str}"),
    };

    let test_case_path = Utf8Path::new(test_case);

    let file_name = test_case_path.file_name();

    let content = fs::read_to_string(test_case_path)
        .expect("Expected test path to be a readable file in UTF8 encoding");

    let parsed = parse_tailwind(&content);
    validate_eof_token(parsed.syntax());

    let formatted_ast = if let Some(ast) = TwRoot::cast(parsed.syntax()) {
        format!("{ast:#?}")
    } else {
        "Failed to parse".to_string()
    };

    let mut snapshot = String::new();
    writeln!(snapshot, "\n## Input\n\n```text\n{content}\n```\n\n").unwrap();

    writeln!(
        snapshot,
        r#"## AST

```
{formatted_ast}
```

## CST

```
{:#?}
```
"#,
        parsed.syntax()
    )
    .unwrap();

    let diagnostics = parsed.diagnostics();
    if !diagnostics.is_empty() {
        let mut diagnostics_buffer = termcolor::Buffer::no_color();

        let termcolor = &mut Termcolor(&mut diagnostics_buffer);
        let mut formatter = Formatter::new(termcolor);

        for diagnostic in diagnostics {
            let error = diagnostic
                .clone()
                .with_file_path(file_name)
                .with_file_source_code(&content);

            formatter
                .write_markup(markup! {
                    {PrintDiagnostic::verbose(&error)}
                })
                .expect("failed to emit diagnostic");
        }

        let formatted_diagnostics =
            std::str::from_utf8(diagnostics_buffer.as_slice()).expect("non utf8 in error buffer");

        if matches!(outcome, ExpectedOutcome::Pass) {
            panic!(
                "Expected no errors to be present in a test case that is expected to pass but the following diagnostics are present:\nCST:\n\n{:?}\n\n{formatted_diagnostics}",
                parsed.syntax()
            )
        }

        writeln!(snapshot, "## Diagnostics\n\n```").unwrap();
        snapshot.write_str(formatted_diagnostics).unwrap();

        writeln!(snapshot, "```\n").unwrap();
    }

    match outcome {
        ExpectedOutcome::Pass => {
            let missing_required = formatted_ast.contains("missing (required)");
            if missing_required
                || parsed
                    .syntax()
                    .descendants()
                    .any(|node| node.kind().is_bogus())
            {
                panic!(
                    "Parsed tree of a 'OK' test case should not contain any missing required children or bogus nodes: \n {formatted_ast:#?} \n\n {formatted_ast}"
                );
            }

            let syntax = parsed.syntax();
            if has_bogus_nodes_or_empty_slots(&syntax) {
                panic!("modified tree has bogus nodes or empty slots:\n{syntax:#?} \n\n {syntax}")
            }
        }
        ExpectedOutcome::Fail => {
            if parsed.diagnostics().is_empty() {
                panic!("Failing test must have diagnostics");
            }
        }
        _ => {}
    }

    insta::with_settings!({
        prepend_module_to_snapshot => false,
        snapshot_path => &test_directory,
    }, {
        insta::assert_snapshot!(file_name, snapshot);
    });
}
