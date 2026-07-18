import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MarkdownPreview } from "./MarkdownPreview";

describe("MarkdownPreview", () => {
  it("does not render raw HTML or executable attributes", () => {
    const markup = renderToStaticMarkup(
      <MarkdownPreview
        source={[
          "# Safe heading",
          '<script>alert("unsafe")</script>',
          '<img src="https://example.com/track" onerror="alert(1)">',
          '<iframe src="file:///etc/passwd"></iframe>',
        ].join("\n\n")}
      />,
    );

    expect(markup).toContain("Safe heading");
    expect(markup).not.toMatch(/<(script|img|iframe)\b/i);
    expect(markup).not.toMatch(/\bonerror=/i);
  });

  it("renders links and images without navigable or fetchable attributes", () => {
    const markup = renderToStaticMarkup(
      <MarkdownPreview
        source={[
          "[remote](https://example.com/path)",
          "[script](javascript:alert(1))",
          "[local file](file:///etc/passwd)",
          "![tracking pixel](https://example.com/track.png)",
        ].join("\n\n")}
      />,
    );

    expect(markup).toContain("remote");
    expect(markup).toContain("script");
    expect(markup).toContain("local file");
    expect(markup).toContain("image blocked");
    expect(markup).not.toMatch(/<(a|img)\b/i);
    expect(markup).not.toMatch(/\b(href|src)=/i);
  });
});
