// ── Lightweight Markdown → JSX ──────────────────────────────────────────────
// Renders a subset of Markdown as SolidJS JSX elements.
// Supports: headings, bold, italic, inline code, fenced code blocks,
// unordered/ordered lists, links.

import { type JSX } from "solid-js";
import { escapeHtml } from "./format";

interface MarkdownToken {
  type: "heading" | "code_block" | "paragraph" | "ul" | "ol";
  level?: number;
  lang?: string;
  content: string;
  items?: string[];
}

export function tokenizeMarkdown(text: string): MarkdownToken[] {
  const lines = text.split("\n");
  const tokens: MarkdownToken[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Fenced code block
    if (line.startsWith("```")) {
      const lang = line.slice(3).trim();
      const codeLines: string[] = [];
      i++;
      while (i < lines.length && !lines[i].startsWith("```")) {
        codeLines.push(lines[i]);
        i++;
      }
      i++; // skip closing ```
      tokens.push({ type: "code_block", lang, content: codeLines.join("\n") });
      continue;
    }

    // Heading
    const headingMatch = line.match(/^(#{1,6})\s+(.+)/);
    if (headingMatch) {
      tokens.push({
        type: "heading",
        level: headingMatch[1].length,
        content: headingMatch[2],
      });
      i++;
      continue;
    }

    // Unordered list
    if (/^[-*+]\s/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^[-*+]\s/.test(lines[i])) {
        items.push(lines[i].replace(/^[-*+]\s/, ""));
        i++;
      }
      tokens.push({ type: "ul", content: "", items });
      continue;
    }

    // Ordered list
    if (/^\d+\.\s/.test(line)) {
      const items: string[] = [];
      while (i < lines.length && /^\d+\.\s/.test(lines[i])) {
        items.push(lines[i].replace(/^\d+\.\s/, ""));
        i++;
      }
      tokens.push({ type: "ol", content: "", items });
      continue;
    }

    // Empty line — skip
    if (!line.trim()) {
      i++;
      continue;
    }

    // Paragraph (collect consecutive non-empty lines)
    const paraLines: string[] = [];
    while (i < lines.length && lines[i].trim() && !lines[i].startsWith("```") && !lines[i].match(/^#{1,6}\s/)) {
      paraLines.push(lines[i]);
      i++;
    }
    tokens.push({ type: "paragraph", content: paraLines.join("\n") });
  }

  return tokens;
}

/** Render inline markdown (bold, italic, code, links) to an HTML string. */
export function renderInlineMarkdown(text: string): string {
  let result = escapeHtml(text);
  // Bold
  result = result.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
  // Italic
  result = result.replace(/\*(.+?)\*/g, "<em>$1</em>");
  // Inline code
  result = result.replace(/`([^`]+)`/g, "<code>$1</code>");
  // Links
  result = result.replace(
    /\[([^\]]+)\]\(([^)]+)\)/g,
    '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>',
  );
  return result;
}

/** Render full markdown text to an HTML string (for use with innerHTML). */
export function renderMarkdownToHtml(text: string): string {
  const tokens = tokenizeMarkdown(text);
  const parts: string[] = [];

  for (const token of tokens) {
    switch (token.type) {
      case "heading": {
        const tag = `h${token.level}`;
        parts.push(`<${tag}>${renderInlineMarkdown(token.content)}</${tag}>`);
        break;
      }
      case "code_block":
        parts.push(
          `<pre><code class="lang-${escapeHtml(token.lang ?? "")}">${escapeHtml(token.content)}</code></pre>`,
        );
        break;
      case "ul":
        parts.push(
          `<ul>${(token.items ?? []).map((item) => `<li>${renderInlineMarkdown(item)}</li>`).join("")}</ul>`,
        );
        break;
      case "ol":
        parts.push(
          `<ol>${(token.items ?? []).map((item) => `<li>${renderInlineMarkdown(item)}</li>`).join("")}</ol>`,
        );
        break;
      case "paragraph":
        parts.push(`<p>${renderInlineMarkdown(token.content)}</p>`);
        break;
    }
  }

  return parts.join("");
}
