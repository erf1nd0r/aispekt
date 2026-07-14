export default {
  fetch(request) {
    const url = new URL(request.url);
    return Response.redirect("https://aispekt.erfindor.com" + url.pathname + url.search, 301);
  },
};
