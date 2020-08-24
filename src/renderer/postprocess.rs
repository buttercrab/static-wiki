use crate::renderer::markdown;
use crate::util::path;
use crate::util::string;
use regex::{Captures, Regex};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use url::Url;

pub fn fix_header<S: AsRef<str>>(html: S) -> String {
    let mut counter = vec![0usize];
    let mut toc = String::new();
    let mut toc_level = 1usize;

    let s = Regex::new(r"<h(\d)>(.*?)</h\d>")
        .unwrap()
        .replace_all(html.as_ref(), |caps: &Captures<'_>| {
            let level = caps[1].parse().unwrap();
            let content = &caps[2];

            while counter.len() < level - 1 {
                counter.push(0);
            }

            while counter.len() > level - 1 {
                counter.pop();
            }

            if counter.len() > 0 {
                *counter.last_mut().unwrap() += 1;
            }

            let mut number = String::new();
            let mut id = String::from("s-");

            for x in counter.iter() {
                number.push_str(&format!("{}.", x));
                id.push_str(&*format!("{}-", x));
            }

            let typing = string::typing_effect(string::typing_process(string::unescape_html(content)))
                .iter()
                .map(|s| string::escape_html(s))
                .collect::<Vec<_>>();

            if level == 1 {
                format!(
                    r##"
<script>
function sleep(ms) {{
  return new Promise(resolve => setTimeout(resolve, ms));
}}

async function writeTitle() {{
    let s = {:?};
    let title = document.getElementsByClassName('title')[0];
    for(let i in s) {{
        title.innerHTML = s[i];
        await sleep(100);
    }}
}}

addOnload(writeTitle);
</script>
<h{level} class="header title">_</h{level}>"##,
                    typing,
                    level = level,
                )
            } else {
                if toc_level < level {
                    while toc_level < level {
                        toc.push_str(r"<ul><li>");
                        toc_level += 1;
                    }
                } else {
                    while toc_level > level {
                        toc.push_str(r"</li></ul>");
                        toc_level -= 1;
                    }
                    toc.push_str(r"</li><li>");
                }

                let regex = Regex::new(r#"<a.+>(.+?)</a>"#).unwrap();
                let text = regex.replace_all(content, |cap: &Captures<'_>| cap[1].to_string()).into_owned();

                toc.push_str(&*format!(
                    r##"<a href="#{id}">{number} {text}</a>"##,
                    id = id,
                    number = number,
                    text = text,
                ));

                format!(
                    r##"<h{level}><a class="header" href="#{id}" id="{id}">{number} {text}</a></h{level}>"##,
                    level = level,
                    id = id,
                    number = number,
                    text = content
                )
            }
        })
        .into_owned();

    while toc_level > 1 {
        toc.push_str(r"</li></ul>");
        toc_level -= 1;
    }

    s.replace(r"<!-- :toc: -->", &*toc)
}

pub fn fix_link<S: AsRef<str>, P: AsRef<Path>>(
    html: S,
    path: P,
    file_map: &HashMap<String, String>,
    titles: &HashSet<String>,
) -> String {
    let html = html.as_ref();
    let path = path.as_ref();

    let html = Regex::new(r#"(<img [^>]*?src=")([^"]+?)""#)
        .unwrap()
        .replace_all(html, |caps: &Captures<'_>| {
            let img_link = &caps[2];

            if Regex::new(r"^[a-z][a-z0-9+.-]*:")
                .unwrap()
                .is_match(img_link)
            {
                panic!(format!(
                    "Image link {} from {:?} that is using outer link",
                    img_link, path
                ));
            }

            if img_link.starts_with('/') {
                panic!(format!(
                    "Image link {} from {:?} that is using absolute path",
                    img_link, path
                ))
            }

            let img_path = path::path_to_str(path::simplify(path.parent().unwrap().join(img_link)));
            let to = file_map
                .get(&*img_path)
                .expect(&*format!("No image {:?} from {:?}", img_link, path));

            format!("{}/{}\"", &caps[1], to)
        })
        .to_string();

    let html = Regex::new(r#"<a (.*?)>"#)
        .unwrap()
        .replace_all(&*html, |caps: &Captures<'_>| {
            let attrs = &caps[1];
            let href = &Regex::new(r#"href="(.*?)""#)
                .unwrap()
                .captures(attrs)
                .unwrap()[1];

            if Regex::new(r"^[a-z][a-z0-9+.-]*:").unwrap().is_match(href) {
                if Regex::new(r#"class="(.*?)""#).unwrap().is_match(attrs) {
                    let attrs = Regex::new(r#"class="(.*?)""#)
                        .unwrap()
                        .replace_all(attrs, |caps: &Captures<'_>| {
                            let mut class = caps[1].to_string();
                            class.push_str(" outer");
                            format!(r#"class="{}""#, class)
                        })
                        .to_string();

                    format!(r#"<a {attrs}>"#, attrs = attrs)
                } else {
                    format!(r#"<a {attrs} class="outer">"#, attrs = attrs)
                }
            } else {
                caps[0].to_string()
            }
        })
        .to_string();

    let html = Regex::new(r"\[\[ +(.+?) +]]")
        .unwrap()
        .replace_all(&*html, |caps: &Captures<'_>| {
            let title = caps[1]
                .to_string()
                .replace(r"\\", r"\")
                .replace(r"\[", r"[")
                .replace(r"\]", r"]");

            let url = Url::parse(&*format!("https://example.com/w/{}", title))
                .unwrap()
                .into_string();
            let href = url.trim_start_matches("https://example.com");

            if titles.contains(&*title) {
                format!(
                    r##"<a href="{href}">{title}</a>"##,
                    href = href,
                    title = title
                )
            } else {
                format!(
                    r##"<a class="no-link" href="{href}">{title}</a>"##,
                    href = href,
                    title = title
                )
            }
        })
        .to_string();

    html
}

pub fn add_github_info<P: AsRef<Path>, S: AsRef<str>, T: AsRef<str>>(
    data: &mut serde_json::Map<String, serde_json::Value>,
    from: P,
    title: S,
    github_url: T,
) {
    let from = from.as_ref();
    let title = title.as_ref();
    let github_url = github_url.as_ref();

    data.insert(
        "time".to_string(),
        json!(format!("최근 수정 시각: {}", markdown::get_time(from))),
    );
    data.insert(
        "github_contributors".to_string(),
        json!(markdown::get_contributors_html(from, github_url)),
    );
    data.insert(
        "github_history".to_string(),
        json!(markdown::get_github_history(from, github_url)),
    );
    data.insert(
        "github_edit".to_string(),
        json!(markdown::get_github_edit(from, github_url)),
    );
    data.insert(
        "github_view_issue".to_string(),
        json!(markdown::get_github_view_issue(github_url, title)),
    );
    data.insert(
        "github_make_issue".to_string(),
        json!(markdown::get_github_make_issue(github_url, title)),
    );
    data.insert(
        "view_in_github".to_string(),
        json!(markdown::get_view_in_github(from, github_url)),
    );
    data.insert(
        "view_in_github_mobile".to_string(),
        json!(markdown::get_view_in_github_mobile(from, github_url)),
    );
    data.insert(
        "github_make_issue_mobile".to_string(),
        json!(markdown::get_github_make_issue_mobile(github_url, title)),
    );
    data.insert(
        "github_view_issue_mobile".to_string(),
        json!(markdown::get_github_view_issue_mobile(github_url, title)),
    );
}

pub fn fix_footnotes<S: AsRef<str>>(html: S) -> String {
    let html = html.as_ref();

    let html = Regex::new(
        r##"(<div class="footnote-definition" id=")(.*?)("><sup class="footnote-definition-label">)(.*?)</sup>([\s\S]*?)</div>"##,
    )
    .unwrap()
    .replace_all(html, |caps: &Captures<'_>| {
        format!(r##"{}f-{id}{}<a href="#b-{id}">{}</a></sup>{}</div>"##, &caps[1], &caps[3], &caps[4], &caps[5], id = &caps[2])
    })
    .to_string();

    let html =
        Regex::new(r##"(<sup class="footnote-reference"><a href="#)(.*?)">([\s\S]*?)</a></sup>"##)
            .unwrap()
            .replace_all(&*html, |caps: &Captures<'_>| {
                format!(
                    r##"{}f-{id}" id="b-{id}">{}</a></sup>"##,
                    &caps[1],
                    &caps[3],
                    id = &caps[2]
                )
            })
            .to_string();

    html
}
