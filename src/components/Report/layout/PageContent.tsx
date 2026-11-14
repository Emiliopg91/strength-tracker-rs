import { JSX, ReactNode } from "react";

type Props = {
    children: ReactNode;
};

export default function PageContent({ children }: Props): JSX.Element {
    return <div className="page-break">{children}</div>;
}
