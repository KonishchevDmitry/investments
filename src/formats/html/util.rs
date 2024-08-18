use diesel::expression::is_aggregate::No;
#[cfg(test)] use indoc::indoc;
#[cfg(test)] use scraper::Html;
use scraper::{ElementRef, Selector};
use scraper::node::Node;
use scraper::selectable::Selectable;

use crate::core::GenericResult;
use crate::util;

// XXX(konishchev): HERE
pub fn new_selector(expression: &str) -> GenericResult<Selector> {
    Ok(Selector::parse(expression).map_err(|e| format!(
        "Invalid HTML selector ({expression}): {e}"))?)
}

// XXX(konishchev): HERE
pub fn select_one<'a>(element: ElementRef<'a>, selector_expression: &str) -> GenericResult<ElementRef<'a>> {
    let selector = new_selector(selector_expression)?;
    let mut selection = element.select(&selector);

    Ok(match (selection.next(), selection.next()) {
        (Some(inner), None) => inner,
        (Some(_), Some(_)) => return Err!("Got multiple elements that match {selector_expression:?} selector"),
        _ => return Err!("There is no element that matches {selector_expression:?} selector"),
    })
}

// XXX(konishchev): HERE
pub fn select_multiple<'a>(element: ElementRef<'a>, selector_expression: &str) -> GenericResult<Vec<ElementRef<'a>>> {
    let selector = new_selector(selector_expression)?;
    Ok(element.select(&selector).collect())
}

pub fn textify(element: ElementRef) -> String {
    let mut text = String::new();

    for node in element.descendants() {
        match node.value() {
            Node::Element(element) if element.name() == "br" => text.push(' '),
            Node::Text(inner) => text.push_str(inner),
            _ => {},
        }
    }

    util::fold_spaces(&text).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textification() {
        let html = indoc!(r#"
            <h3 style="line-height:1.0;" align="center">
                <br>Отчет брокера<br>за период с 13.08.2024 по 13.08.2024, дата создания 14.08.2024<br>
                <span>some <i>nested</i> text</span>
            </h3>
        "#);

        // XXX(konishchev): HERE
        assert_eq!(
            textify(Html::parse_fragment(html).root_element()),
            "Отчет брокера за период с 13.08.2024 по 13.08.2024, дата создания 14.08.2024 some nested text",
        );
    }
}