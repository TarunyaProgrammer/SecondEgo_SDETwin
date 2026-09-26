"use client";

import { motion, useReducedMotion } from "framer-motion";

export type PipelineNode = {
  label: string;
  detail?: string;
  state?: "default" | "active" | "complete" | "warning";
};

type PipelineDiagramProps = {
  nodes: PipelineNode[];
  label: string;
  className?: string;
};

export function PipelineDiagram({ nodes, label, className = "" }: PipelineDiagramProps) {
  const reduceMotion = useReducedMotion();

  return (
    <div className={`pipeline-diagram ${className}`} role="img" aria-label={label}>
      <ol className="pipeline-diagram__nodes">
        {nodes.map((node, index) => (
          <motion.li
            className={`pipeline-node pipeline-node--${node.state ?? "default"}`}
            key={`${node.label}-${index}`}
            initial={reduceMotion ? false : { opacity: 0, y: 8 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true, amount: 0.4 }}
            transition={{ duration: reduceMotion ? 0 : 0.35, delay: reduceMotion ? 0 : index * 0.09 }}
          >
            <span className="pipeline-node__top"><span className="pipeline-node__index">0{index + 1}</span><span className="pipeline-node__dot" /></span>
            <span className="pipeline-node__label">{node.label}</span>
            {node.detail && <span className="pipeline-node__detail">{node.detail}</span>}
            {index < nodes.length - 1 && <span className="pipeline-node__connector" aria-hidden="true">↗</span>}
          </motion.li>
        ))}
      </ol>
    </div>
  );
}
