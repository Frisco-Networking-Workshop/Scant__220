

c = console.log.bind console
express = require 'express'
app = express()
app.use (express.static '../dist', null)
app.listen 8001, ->
    c 'listening on 8001'
