// XXX(konishchev): Rewrite
use scraper::{selectable::Selectable, ElementRef, Selector};

use crate::core::GenericResult;

pub fn new_selector(expression: &str) -> GenericResult<Selector> {
    Ok(Selector::parse(expression).map_err(|e| format!(
        "Invalid HTML selector ({expression}): {e}"))?)
}

pub fn select_one<'a>(element: ElementRef<'a>, selector_expression: &str) -> GenericResult<ElementRef<'a>> {
    let selector = new_selector(selector_expression)?;
    let mut selection = element.select(&selector);

    Ok(match (selection.next(), selection.next()) {
        (Some(inner), None) => inner,
        (Some(_), Some(_)) => return Err!("Got multiple elements that match {selector_expression:?} selector"),
        _ => return Err!("There is no element that matches {selector_expression:?} selector"),
    })
}

pub fn select_multiple<'a>(element: ElementRef<'a>, selector_expression: &str) -> GenericResult<Vec<ElementRef<'a>>> {
    let selector = new_selector(selector_expression)?;
    Ok(element.select(&selector).collect())
}