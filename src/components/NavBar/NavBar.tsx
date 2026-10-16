import { JSX, memo } from "react";
import { Box } from "@mui/material";
import "@/styles/NavBar/NavBar.css";

export type NavBarItem = {
    label: JSX.Element;
    selected: boolean;
    onSelected: () => void;
};

function NavBarInner({
    leftItems,
    rightItems,
}: {
    leftItems: NavBarItem[];
    rightItems: NavBarItem[];
}): JSX.Element {
    const renderItem = (item: NavBarItem, idx: number) => (
        <Box
            key={"navbar-" + idx}
            onClick={item.onSelected}
            className={`navbar-item${item.selected ? " navbar-item-selected" : ""}`}
        >
            {item.label}
        </Box>
    );

    return (
        <Box className="navbar">
            {leftItems.map(renderItem)}
            {leftItems.length > 0 && rightItems.length > 0 && (
                <Box key="separator" className="navbar-separator" />
            )}
            {rightItems.map(renderItem)}
        </Box>
    );
}

export const NavBar = memo(NavBarInner);
