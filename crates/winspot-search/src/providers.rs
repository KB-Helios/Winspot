use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use winspot_core::{ActionKind, SearchResult, SearchResultKind};

pub trait SearchProvider {
    fn collect_results(&self) -> Vec<SearchResult>;
}

pub trait DynamicSearchProvider: Send + Sync {
    fn search(&self, query: &str) -> Vec<SearchResult>;
}

const DEFAULT_FILE_SYSTEM_MAX_DEPTH: usize = 2;
const DEFAULT_FILE_SYSTEM_MAX_ENTRIES: usize = 500;

const WINDOWS_SETTINGS: &[WindowsSetting] = &[
    WindowsSetting {
        title: "Settings",
        subtitle: "Open Windows Settings",
        uri: "ms-settings:",
    },
    WindowsSetting {
        title: "Display settings",
        subtitle: "Windows Settings > System > Display",
        uri: "ms-settings:display",
    },
    WindowsSetting {
        title: "Sound settings",
        subtitle: "Windows Settings > System > Sound",
        uri: "ms-settings:sound",
    },
    WindowsSetting {
        title: "Bluetooth settings",
        subtitle: "Windows Settings > Bluetooth & devices",
        uri: "ms-settings:bluetooth",
    },
    WindowsSetting {
        title: "Network settings",
        subtitle: "Windows Settings > Network & internet",
        uri: "ms-settings:network",
    },
    WindowsSetting {
        title: "Apps settings",
        subtitle: "Windows Settings > Apps > Installed apps",
        uri: "ms-settings:appsfeatures",
    },
    WindowsSetting {
        title: "Startup apps",
        subtitle: "Windows Settings > Apps > Startup",
        uri: "ms-settings:startupapps",
    },
    WindowsSetting {
        title: "Default apps",
        subtitle: "Windows Settings > Apps > Default apps",
        uri: "ms-settings:defaultapps",
    },
    WindowsSetting {
        title: "Privacy settings",
        subtitle: "Windows Settings > Privacy & security",
        uri: "ms-settings:privacy",
    },
    WindowsSetting {
        title: "Windows Update",
        subtitle: "Windows Settings > Windows Update",
        uri: "ms-settings:windowsupdate",
    },
];

struct WindowsSetting {
    title: &'static str,
    subtitle: &'static str,
    uri: &'static str,
}

const UNIT_DEFINITIONS: &[UnitDefinition] = &[
    UnitDefinition {
        aliases: &["m", "meter", "meters", "metre", "metres"],
        symbol: "m",
        dimension: UnitDimension::Length,
        scale: UnitScale::Linear { to_base: 1.0 },
    },
    UnitDefinition {
        aliases: &["km", "kilometer", "kilometers", "kilometre", "kilometres"],
        symbol: "km",
        dimension: UnitDimension::Length,
        scale: UnitScale::Linear { to_base: 1000.0 },
    },
    UnitDefinition {
        aliases: &["cm", "centimeter", "centimeters", "centimetre", "centimetres"],
        symbol: "cm",
        dimension: UnitDimension::Length,
        scale: UnitScale::Linear { to_base: 0.01 },
    },
    UnitDefinition {
        aliases: &["mi", "mile", "miles"],
        symbol: "mi",
        dimension: UnitDimension::Length,
        scale: UnitScale::Linear { to_base: 1609.344 },
    },
    UnitDefinition {
        aliases: &["ft", "foot", "feet"],
        symbol: "ft",
        dimension: UnitDimension::Length,
        scale: UnitScale::Linear { to_base: 0.3048 },
    },
    UnitDefinition {
        aliases: &["in", "inch", "inches"],
        symbol: "in",
        dimension: UnitDimension::Length,
        scale: UnitScale::Linear { to_base: 0.0254 },
    },
    UnitDefinition {
        aliases: &["kg", "kilogram", "kilograms"],
        symbol: "kg",
        dimension: UnitDimension::Mass,
        scale: UnitScale::Linear { to_base: 1.0 },
    },
    UnitDefinition {
        aliases: &["g", "gram", "grams"],
        symbol: "g",
        dimension: UnitDimension::Mass,
        scale: UnitScale::Linear { to_base: 0.001 },
    },
    UnitDefinition {
        aliases: &["lb", "lbs", "pound", "pounds"],
        symbol: "lb",
        dimension: UnitDimension::Mass,
        scale: UnitScale::Linear {
            to_base: 0.45359237,
        },
    },
    UnitDefinition {
        aliases: &["oz", "ounce", "ounces"],
        symbol: "oz",
        dimension: UnitDimension::Mass,
        scale: UnitScale::Linear {
            to_base: 0.028349523125,
        },
    },
    UnitDefinition {
        aliases: &["c", "celsius"],
        symbol: "C",
        dimension: UnitDimension::Temperature,
        scale: UnitScale::Celsius,
    },
    UnitDefinition {
        aliases: &["f", "fahrenheit"],
        symbol: "F",
        dimension: UnitDimension::Temperature,
        scale: UnitScale::Fahrenheit,
    },
    UnitDefinition {
        aliases: &["k", "kelvin"],
        symbol: "K",
        dimension: UnitDimension::Temperature,
        scale: UnitScale::Kelvin,
    },
];

#[derive(Debug, Clone, Copy)]
struct UnitDefinition {
    aliases: &'static [&'static str],
    symbol: &'static str,
    dimension: UnitDimension,
    scale: UnitScale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitDimension {
    Length,
    Mass,
    Temperature,
}

#[derive(Debug, Clone, Copy)]
enum UnitScale {
    Linear { to_base: f64 },
    Celsius,
    Fahrenheit,
    Kelvin,
}

#[derive(Debug, Default)]
pub struct BuiltinCommandProvider;

impl BuiltinCommandProvider {
    pub fn collect_results(&self) -> Vec<SearchResult> {
        <Self as SearchProvider>::collect_results(self)
    }
}

impl SearchProvider for BuiltinCommandProvider {
    fn collect_results(&self) -> Vec<SearchResult> {
        vec![
            SearchResult {
                id: "command:calculator".to_string(),
                title: "Calculator".to_string(),
                subtitle: "Built-in command".to_string(),
                kind: SearchResultKind::Command,
                score: 0.0,
                primary_action: ActionKind::RunCommand,
            },
            SearchResult {
                id: "command:terminal".to_string(),
                title: "Terminal".to_string(),
                subtitle: "Open a shell command".to_string(),
                kind: SearchResultKind::Command,
                score: 0.0,
                primary_action: ActionKind::RunCommand,
            },
        ]
    }
}

#[derive(Debug, Default)]
pub struct WindowsSettingsProvider;

impl WindowsSettingsProvider {
    pub fn collect_results(&self) -> Vec<SearchResult> {
        <Self as SearchProvider>::collect_results(self)
    }
}

impl SearchProvider for WindowsSettingsProvider {
    fn collect_results(&self) -> Vec<SearchResult> {
        WINDOWS_SETTINGS
            .iter()
            .map(|setting| SearchResult {
                id: format!("setting:{}", setting.uri),
                title: setting.title.to_string(),
                subtitle: setting.subtitle.to_string(),
                kind: SearchResultKind::Setting,
                score: 0.0,
                primary_action: ActionKind::Open,
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct StartMenuAppProvider {
    roots: Vec<PathBuf>,
}

impl Default for StartMenuAppProvider {
    fn default() -> Self {
        Self::new(default_start_menu_roots())
    }
}

impl StartMenuAppProvider {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    pub fn collect_results(&self) -> Vec<SearchResult> {
        <Self as SearchProvider>::collect_results(self)
    }
}

impl SearchProvider for StartMenuAppProvider {
    fn collect_results(&self) -> Vec<SearchResult> {
        let mut results = Vec::new();
        for root in &self.roots {
            collect_shortcuts(root, 0, &mut results);
        }
        results
    }
}

#[derive(Debug, Clone)]
pub struct FileSystemProvider {
    roots: Vec<PathBuf>,
    max_depth: usize,
    max_entries: usize,
}

impl Default for FileSystemProvider {
    fn default() -> Self {
        Self::new(
            default_file_system_roots(),
            DEFAULT_FILE_SYSTEM_MAX_DEPTH,
            DEFAULT_FILE_SYSTEM_MAX_ENTRIES,
        )
    }
}

impl FileSystemProvider {
    pub fn new(roots: Vec<PathBuf>, max_depth: usize, max_entries: usize) -> Self {
        Self {
            roots,
            max_depth,
            max_entries,
        }
    }

    pub fn collect_results(&self) -> Vec<SearchResult> {
        <Self as SearchProvider>::collect_results(self)
    }
}

impl SearchProvider for FileSystemProvider {
    fn collect_results(&self) -> Vec<SearchResult> {
        let mut results = Vec::new();
        for root in &self.roots {
            collect_file_system_entries(root, 0, self.max_depth, self.max_entries, &mut results);
            if results.len() >= self.max_entries {
                break;
            }
        }
        results
    }
}

#[derive(Debug, Default)]
pub struct RunningProcessProvider;

impl RunningProcessProvider {
    pub fn collect_results(&self) -> Vec<SearchResult> {
        <Self as SearchProvider>::collect_results(self)
    }

    pub fn collect_from_tasklist_csv(output: &str) -> Vec<SearchResult> {
        parse_tasklist_csv(output)
    }
}

impl SearchProvider for RunningProcessProvider {
    fn collect_results(&self) -> Vec<SearchResult> {
        if !cfg!(windows) {
            return Vec::new();
        }

        let Ok(output) = Command::new("tasklist").args(["/FO", "CSV", "/NH"]).output() else {
            return Vec::new();
        };

        if !output.status.success() {
            return Vec::new();
        }

        let csv = String::from_utf8_lossy(&output.stdout);
        parse_tasklist_csv(&csv)
    }
}

#[derive(Debug, Default)]
pub struct CalculatorProvider;

impl DynamicSearchProvider for CalculatorProvider {
    fn search(&self, query: &str) -> Vec<SearchResult> {
        let expression = query.trim();
        let Some(value) = evaluate_arithmetic_expression(expression) else {
            return Vec::new();
        };

        vec![SearchResult {
            id: format!("calculator:{expression}"),
            title: format!("{expression} = {}", format_number(value)),
            subtitle: "Calculator result".to_string(),
            kind: SearchResultKind::Command,
            score: 0.0,
            primary_action: ActionKind::Copy,
        }]
    }
}

#[derive(Debug, Default)]
pub struct UnitConversionProvider;

impl DynamicSearchProvider for UnitConversionProvider {
    fn search(&self, query: &str) -> Vec<SearchResult> {
        let Some(conversion) = parse_unit_conversion(query) else {
            return Vec::new();
        };

        vec![SearchResult {
            id: format!(
                "conversion:{}:{}:{}",
                conversion.input_value, conversion.from.symbol, conversion.to.symbol
            ),
            title: format!(
                "{} {} to {} = {} {}",
                format_number(conversion.input_value),
                conversion.from.symbol,
                conversion.to.symbol,
                format_number(conversion.output_value),
                conversion.to.symbol
            ),
            subtitle: "Unit conversion".to_string(),
            kind: SearchResultKind::Command,
            score: 0.0,
            primary_action: ActionKind::Copy,
        }]
    }
}

fn default_start_menu_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(program_data) = env::var("ProgramData") {
        roots.push(PathBuf::from(program_data).join("Microsoft\\Windows\\Start Menu"));
    }
    if let Ok(app_data) = env::var("APPDATA") {
        roots.push(PathBuf::from(app_data).join("Microsoft\\Windows\\Start Menu"));
    }
    roots
}

fn default_file_system_roots() -> Vec<PathBuf> {
    let Ok(user_profile) = env::var("USERPROFILE") else {
        return Vec::new();
    };

    let user_profile = PathBuf::from(user_profile);
    ["Desktop", "Documents", "Downloads"]
        .into_iter()
        .map(|folder| user_profile.join(folder))
        .collect()
}

fn collect_shortcuts(root: &Path, results: &mut Vec<SearchResult>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_shortcuts(&path, results);
            continue;
        }

        if path.extension().and_then(|extension| extension.to_str()) != Some("lnk") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        results.push(SearchResult {
            id: format!("app:{}", path.display()),
            title: stem.to_string(),
            subtitle: path.display().to_string(),
            kind: SearchResultKind::App,
            score: 0.0,
            primary_action: ActionKind::Open,
        });
    }
}

fn evaluate_arithmetic_expression(expression: &str) -> Option<f64> {
    if !looks_like_math(expression) {
        return None;
    }

    let mut parser = ArithmeticParser::new(expression);
    let value = parser.parse_expression()?;
    parser.skip_whitespace();
    if parser.is_finished() && value.is_finite() {
        Some(value)
    } else {
        None
    }
}

fn looks_like_math(expression: &str) -> bool {
    let mut has_digit = false;
    let mut has_operator = false;
    for character in expression.chars() {
        if character.is_ascii_digit() {
            has_digit = true;
            continue;
        }

        if matches!(character, '+' | '-' | '*' | '/' | '.' | '(' | ')' | ' ') {
            has_operator |= matches!(character, '+' | '-' | '*' | '/');
            continue;
        }

        return false;
    }

    has_digit && has_operator
}

fn format_number(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        let formatted = format!("{value:.6}");
        formatted
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

struct UnitConversion {
    input_value: f64,
    output_value: f64,
    from: &'static UnitDefinition,
    to: &'static UnitDefinition,
}

fn parse_unit_conversion(query: &str) -> Option<UnitConversion> {
    let normalized = query.trim().to_lowercase();
    let (input_value, rest) = parse_leading_number(&normalized)?;
    let (from_text, to_text) = split_conversion_units(rest.trim())?;
    let from = find_unit(from_text)?;
    let to = find_unit(to_text)?;
    let output_value = convert_units(input_value, from, to)?;

    Some(UnitConversion {
        input_value,
        output_value,
        from,
        to,
    })
}

fn parse_leading_number(input: &str) -> Option<(f64, &str)> {
    let mut end = 0;
    let mut has_digit = false;

    for (index, character) in input.char_indices() {
        if character.is_ascii_digit() {
            has_digit = true;
            end = index + character.len_utf8();
            continue;
        }

        if matches!(character, '+' | '-' | '.') {
            end = index + character.len_utf8();
            continue;
        }

        break;
    }

    if !has_digit || end == 0 {
        return None;
    }

    let value = input[..end].parse().ok()?;
    Some((value, &input[end..]))
}

fn split_conversion_units(input: &str) -> Option<(&str, &str)> {
    input
        .split_once(" to ")
        .or_else(|| input.split_once(" in "))
        .map(|(from, to)| (from.trim(), first_unit_token(to.trim())))
        .filter(|(from, to)| !from.is_empty() && !to.is_empty())
}

fn first_unit_token(input: &str) -> &str {
    input.split_whitespace().next().unwrap_or(input)
}

fn find_unit(input: &str) -> Option<&'static UnitDefinition> {
    let unit = first_unit_token(input.trim());
    UNIT_DEFINITIONS
        .iter()
        .find(|definition| definition.aliases.contains(&unit))
}

fn convert_units(
    value: f64,
    from: &'static UnitDefinition,
    to: &'static UnitDefinition,
) -> Option<f64> {
    if from.dimension != to.dimension {
        return None;
    }

    match (from.scale, to.scale) {
        (UnitScale::Linear { to_base: from_base }, UnitScale::Linear { to_base }) => {
            Some(value * from_base / to_base)
        }
        _ => {
            let celsius = to_celsius(value, from.scale)?;
            from_celsius(celsius, to.scale)
        }
    }
}

fn to_celsius(value: f64, scale: UnitScale) -> Option<f64> {
    match scale {
        UnitScale::Celsius => Some(value),
        UnitScale::Fahrenheit => Some((value - 32.0) * 5.0 / 9.0),
        UnitScale::Kelvin => Some(value - 273.15),
        UnitScale::Linear { .. } => None,
    }
}

fn from_celsius(value: f64, scale: UnitScale) -> Option<f64> {
    match scale {
        UnitScale::Celsius => Some(value),
        UnitScale::Fahrenheit => Some(value * 9.0 / 5.0 + 32.0),
        UnitScale::Kelvin => Some(value + 273.15),
        UnitScale::Linear { .. } => None,
    }
}

struct ArithmeticParser<'a> {
    expression: &'a str,
    offset: usize,
}

impl<'a> ArithmeticParser<'a> {
    fn new(expression: &'a str) -> Self {
        Self {
            expression,
            offset: 0,
        }
    }

    fn parse_expression(&mut self) -> Option<f64> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_whitespace();
            if self.consume('+') {
                value += self.parse_term()?;
            } else if self.consume('-') {
                value -= self.parse_term()?;
            } else {
                return Some(value);
            }
        }
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut value = self.parse_factor()?;
        loop {
            self.skip_whitespace();
            if self.consume('*') {
                value *= self.parse_factor()?;
            } else if self.consume('/') {
                let divisor = self.parse_factor()?;
                if divisor == 0.0 {
                    return None;
                }
                value /= divisor;
            } else {
                return Some(value);
            }
        }
    }

    fn parse_factor(&mut self) -> Option<f64> {
        self.skip_whitespace();
        if self.consume('(') {
            let value = self.parse_expression()?;
            self.skip_whitespace();
            return self.consume(')').then_some(value);
        }

        let start = self.offset;
        if self.peek() == Some('-') {
            self.offset += 1;
        }

        while matches!(self.peek(), Some(character) if character.is_ascii_digit() || character == '.')
        {
            self.offset += 1;
        }

        if self.offset == start || self.expression[start..self.offset] == *"-" {
            return None;
        }

        self.expression[start..self.offset].parse().ok()
    }

    fn skip_whitespace(&mut self) {
        while self.peek() == Some(' ') {
            self.offset += 1;
        }
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.offset += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<char> {
        self.expression[self.offset..].chars().next()
    }

    fn is_finished(&self) -> bool {
        self.offset == self.expression.len()
    }
}

fn collect_file_system_entries(
    root: &Path,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    results: &mut Vec<SearchResult>,
) {
    if depth >= max_depth || results.len() >= max_entries {
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        if results.len() >= max_entries {
            return;
        }

        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        let Some(title) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            continue;
        };

        if file_type.is_dir() {
            results.push(SearchResult {
                id: format!("folder:{}", path.display()),
                title: title.to_string(),
                subtitle: path.display().to_string(),
                kind: SearchResultKind::Folder,
                score: 0.0,
                primary_action: ActionKind::Open,
            });
            collect_file_system_entries(&path, depth + 1, max_depth, max_entries, results);
            continue;
        }

        if file_type.is_file() {
            results.push(SearchResult {
                id: format!("file:{}", path.display()),
                title: title.to_string(),
                subtitle: path.display().to_string(),
                kind: SearchResultKind::File,
                score: 0.0,
                primary_action: ActionKind::Open,
            });
        }
    }
}

fn parse_tasklist_csv(output: &str) -> Vec<SearchResult> {
    output
        .lines()
        .filter_map(parse_process_line)
        .collect()
}

fn parse_process_line(line: &str) -> Option<SearchResult> {
    let columns = parse_csv_line(line);
    if columns.len() < 5 {
        return None;
    }

    let image_name = columns[0].trim();
    let pid = columns[1].trim();
    let memory = columns[4].trim();

    if image_name.is_empty() || pid.is_empty() || image_name == "Image Name" {
        return None;
    }

    Some(SearchResult {
        id: format!("process:{pid}:{image_name}"),
        title: image_name.to_string(),
        subtitle: format!("PID {pid} - {memory}"),
        kind: SearchResultKind::Process,
        score: 0.0,
        primary_action: ActionKind::Copy,
    })
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut columns = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_quotes = false;

    while let Some(character) = chars.next() {
        match character {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                columns.push(current);
                current = String::new();
            }
            _ => current.push(character),
        }
    }

    columns.push(current);
    columns
}
