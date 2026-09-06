local native = _native
local function id(o) if type(o)=='table' then return o._id else return o end end
local methods = {}
local mt = { __index=methods, __eq=function(a,b) return a._id==b._id end }
local function object(n) n=id(n); native.get(n,'name'); return setmetatable({_id=n},mt) end
function methods:dbref() return self._id end
function methods:name() return native.get(self._id,'name') end
function methods:type() return native.get(self._id,'type') end
function methods:description() return native.get(self._id,'description') end
function methods:affiliation() local n=native.get(self._id,'affiliation'); if n then return object(n) end end
function methods:set_name(s) native.set(self._id,'name',s) end
function methods:set_description(s) native.set(self._id,'description',s) end
function methods:internal_description() return native.get(self._id,'internal_description') end
function methods:set_internal_description(s) native.set(self._id,'internal_description',s) end
function methods:set_home(o) native.set(self._id,'home',id(o)) end
function methods:contents(opts)
 opts=opts or {}; local r={}; for _,n in ipairs(native.contents(self._id,opts.types or {},id(opts.visible_to))) do r[#r+1]=object(n) end; return r
end
function methods:flags()
 local n=self._id
 return {has=function(_,flag) return native.has_flag(n,flag) end,
 add=function(_,flag) return native.flag(n,flag,true) end,remove=function(_,flag) return native.flag(n,flag,false) end}
end
function methods:powers()
 local n=self._id
 return {has=function(_,power) return native.has_power(n,power) end,
 add=function(_,power) return native.power(n,power,true) end,
 remove=function(_,power) return native.power(n,power,false) end}
end
function methods:state(ns)
 local n=self._id
 return {entries=function() return native.entries(n,ns) end,
 get=function(_,key,default) for _,e in ipairs(native.entries(n,ns)) do if e.key==key then return e.value end end return default end,
 set=function(_,key,v) native.state_set(n,ns,key,v) end}
end
local flags=native.flags
mux={world={types={ROOM=0,THING=1,EXIT=2,PLAYER=3},flags=flags,powers=native.powers,locks={TRAVERSE='traverse',TELEPORT='teleport',TELEPORT_OUT='teleport_out'},object=object},session={},config={get=native.config},text={markdown=native.markdown,is_printable_ascii=native.printable_ascii,markup=native.markup,style=native.style,width=native.width,truncate=native.truncate,strip_style=native.strip},comsys={flags={PUBLIC=1}}}
function mux.world.create_object(t)
 local copy={};for k,v in pairs(t) do copy[k]=id(v) end
 return object(native.create(copy))
end
function mux.world.teleport_object(t) native.set(id(t.object),'location',id(t.destination)) end
function mux.world.pemit(o,s) native.pemit(id(o),s) end
function mux.world.lock_passes(t)
 local n=id(t.object);local parent=_parents[_object_parents[n]]
 if not parent then error('Missing lock parent') end
 local lock=parent.locks and parent.locks[t.lock]
 if lock==nil then return true end
 local result=lock({object=n,subject=id(t.subject or t.enactor),enactor=id(t.enactor),cause=id(t.cause or t.enactor),source=id(t.source),destination=id(t.destination),descriptor=t.descriptor})
 return result==true or (type(result)=='table' and result.passes==true)
end
function mux.comsys.create_channel(name)
 native.channel(name)
 return {set_object=function(_,o) native.channel(name,id(o)) end,flags=function() return {add=function(_,f) native.channel(name,nil,f) end} end}
end
function mux.session.connected_players()
 local players={}
 for _,p in ipairs(_connected_players or {}) do players[#players+1]={object=object(p.dbref),name=p.name,connected_for=p.connected_for,idle_for=p.idle_for} end
 return players
end
function mux.session.who_summary() return _who_summary or {hidden=0,record=0} end
function mux.session.flow_start() error('Interactive Lua flows are unavailable in this milestone') end
package.loaded.mux=mux

-- Restrict require to game packages and safe built-ins. Do not allow scripts to
-- recover removed runtime libraries through package.loaded or preload.
local original_require=require
local forbidden={io=true,os=true,debug=true,ffi=true,jit=true}
for name in pairs(forbidden) do package.loaded[name]=nil; package.preload[name]=nil end
function require(name)
 if forbidden[name:match('^[^.]+')] or not name:match('^[%w_%.]+$') or name:find('%.%.') then error('Unavailable Lua package '..tostring(name)) end
 return original_require(name)
end
package.loadlib=nil
