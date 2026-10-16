/** Local collaboration data boundary. Native transport and machine lifetime
 * stay behind this module; presentation never constructs native commands. */
export {
  actOnDesktopThread, attachDesktopThread, chooseDesktopItem, createDesktopThread,
  desktopCatalog, openDesktopThread,
  type DesktopAction, type DesktopCatalogItem, type DesktopChosenItem, type DesktopMode,
  type DesktopPendingRequest, type DesktopQuestion, type DesktopView, type DesktopReview
} from "../api/desktop";
