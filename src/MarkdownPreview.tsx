import ReactMarkdown from "react-markdown";

type MarkdownPreviewProps = {
  source: string;
};

export function MarkdownPreview({ source }: MarkdownPreviewProps) {
  return (
    <ReactMarkdown
      skipHtml
      components={{
        a({ children, href }) {
          return (
            <span className="preview-link">
              {children}
              {href && <span className="preview-url"> ({href})</span>}
            </span>
          );
        },
        img({ alt, src }) {
          const description = alt || src || "image";
          return (
            <span className="preview-image" role="img" aria-label={description}>
              [image blocked: {description}]
            </span>
          );
        },
      }}
    >
      {source}
    </ReactMarkdown>
  );
}
