// Auth Middleware
function requireAuth(req, res, next) {
    if (!req.headers.authorization) return res.status(401).send('Unauthorized');
    next();
}
module.exports = { requireAuth };