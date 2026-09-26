type CodeBlockProps = {
  title: string;
  language: string;
  lines: string[];
  className?: string;
};

export function CodeBlock({ title, language, lines, className = "" }: CodeBlockProps) {
  return (
    <div className={`code-window ${className}`}>
      <div className="code-window__top">
        <span className="window-controls" aria-hidden="true"><i /><i /><i /></span>
        <span className="code-window__title">{title}</span>
        <span className="code-window__language">{language}</span>
      </div>
      <pre className="code-window__body"><code>{lines.map((line, index) => (
        <span className="code-line" key={`${index}-${line}`}><span className="code-line__number">{String(index + 1).padStart(2, "0")}</span><span>{line || " "}</span></span>
      ))}</code></pre>
    </div>
  );
}
