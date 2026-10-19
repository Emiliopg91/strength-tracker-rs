import { JSX, ReactNode } from "react";

type Props = {
    children: ReactNode;
};

export default function NoPageBreak({ children }: Props): JSX.Element {
    return <div className="no-page-break">{children}</div>;
}
