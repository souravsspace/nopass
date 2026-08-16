/**
 * A section that arrives as it comes into view.
 *
 * The whole animation vocabulary of this site: eight pixels up, 320ms,
 * ease-out, once. No bounce, no stagger longer than a glance, nothing that
 * has to finish before the words can be read (DESIGN.md §5). Motion's
 * `useReducedMotion` turns it into a plain fade for anyone who asked.
 */

import { motion, useReducedMotion } from "motion/react";
import type { ReactNode } from "react";

export function Reveal({
  children,
  delay = 0,
}: {
  children: ReactNode;
  delay?: number;
}) {
  const still = useReducedMotion();

  return (
    <motion.div
      initial={still ? { opacity: 0 } : { opacity: 0, y: 8 }}
      transition={{ delay, duration: 0.32, ease: "easeOut" }}
      viewport={{ margin: "-64px", once: true }}
      whileInView={{ opacity: 1, y: 0 }}
    >
      {children}
    </motion.div>
  );
}
