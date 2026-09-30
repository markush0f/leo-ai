/**
 * Renders assistant replies as GitHub-flavored markdown.
 * Compiles to React elements (no `dangerouslySetInnerHTML`).
 */
import { memo, type ComponentProps } from "react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import { md } from "./ui";

type Props = { text: string };

const tags = {
  p: (props: ComponentProps<"p">) => <p className={md.p} {...props} />,
  h1: (props: ComponentProps<"h1">) => <h1 className={md.h} {...props} />,
  h2: (props: ComponentProps<"h2">) => <h2 className={md.h} {...props} />,
  h3: (props: ComponentProps<"h3">) => <h3 className={md.hSmall} {...props} />,
  h4: (props: ComponentProps<"h4">) => <h4 className={md.hSmall} {...props} />,
  h5: (props: ComponentProps<"h5">) => <h5 className={md.hSmall} {...props} />,
  h6: (props: ComponentProps<"h6">) => <h6 className={md.hSmall} {...props} />,
  ul: (props: ComponentProps<"ul">) => <ul className={md.ul} {...props} />,
  ol: (props: ComponentProps<"ol">) => <ol className={md.ol} {...props} />,
  blockquote: (props: ComponentProps<"blockquote">) => <blockquote className={md.quote} {...props} />,
  hr: () => <hr className={md.hr} />,
  img: (props: ComponentProps<"img">) => <img className={md.img} {...props} />,
  pre: (props: ComponentProps<"pre">) => <pre className={md.pre} {...props} />,
  code: ({ className, ...props }: ComponentProps<"code">) => <code className={className ? className : md.code} {...props} />,
  input: (props: ComponentProps<"input">) => <input className={props.type === "checkbox" ? md.check : undefined} {...props} />,
  th: (props: ComponentProps<"th">) => <th className={`${md.cell} ${md.th}`} {...props} />,
  td: (props: ComponentProps<"td">) => <td className={md.cell} {...props} />,
};

export const Markdown = memo(function Markdown({ text }: Props) {
  return (
    <div className={md.root}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        urlTransform={defaultUrlTransform}
        components={{
          ...tags,
          a: ({ href, children }) => (
            <a className={md.a} href={href} target="_blank" rel="noreferrer noopener">
              {children}
            </a>
          ),
          table: ({ children }) => (
            <div className={md.tableWrap}>
              <table className={md.table}>{children}</table>
            </div>
          ),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
});
