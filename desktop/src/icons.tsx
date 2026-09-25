import { HugeiconsIcon } from "@hugeicons/react";
import {
  Mic01Icon, ArrowUp02Icon, Add01Icon, Sun03Icon, Moon02Icon,
  Settings02Icon, Menu01Icon, PowerServiceIcon, Cancel01Icon,
  Database02Icon, SidebarLeft01Icon, BubbleChatIcon, Brain01Icon,
  Wrench01Icon, ArrowDown02Icon, ViewIcon, ViewOffSlashIcon,
  Search01Icon, Delete02Icon, Link01Icon, Tick02Icon,
} from "@hugeicons/core-free-icons";

type Props = { className?: string };
const makeIcon = (icon: typeof Mic01Icon) => function Icon(props: Props) {
  return <HugeiconsIcon icon={icon} size={20} strokeWidth={1.7} aria-hidden="true" {...props} />;
};

export const IconMic = makeIcon(Mic01Icon);
export const IconSend = makeIcon(ArrowUp02Icon);
export const IconPlus = makeIcon(Add01Icon);
export const IconSun = makeIcon(Sun03Icon);
export const IconMoon = makeIcon(Moon02Icon);
export const IconSliders = makeIcon(Settings02Icon);
export const IconMenu = makeIcon(Menu01Icon);
export const IconPower = makeIcon(PowerServiceIcon);
export const IconClose = makeIcon(Cancel01Icon);
export const IconDatabase = makeIcon(Database02Icon);
export const IconSidebar = makeIcon(SidebarLeft01Icon);
export const IconChat = makeIcon(BubbleChatIcon);
export const IconBrain = makeIcon(Brain01Icon);
export const IconTools = makeIcon(Wrench01Icon);
export const IconDown = makeIcon(ArrowDown02Icon);
export const IconEye = makeIcon(ViewIcon);
export const IconEyeOff = makeIcon(ViewOffSlashIcon);
export const IconSearch = makeIcon(Search01Icon);
export const IconDelete = makeIcon(Delete02Icon);
export const IconLink = makeIcon(Link01Icon);
export const IconCheck = makeIcon(Tick02Icon);
