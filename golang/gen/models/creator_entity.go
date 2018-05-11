// Code generated by go-swagger; DO NOT EDIT.

package models

// This file was generated by the swagger tool.
// Editing this file might prove futile when you re-run the swagger generate command

import (
	"encoding/json"

	strfmt "github.com/go-openapi/strfmt"

	"github.com/go-openapi/errors"
	"github.com/go-openapi/swag"
	"github.com/go-openapi/validate"
)

// CreatorEntity creator entity
// swagger:model creator_entity
type CreatorEntity struct {

	// ident
	// Required: true
	Ident *string `json:"ident"`

	// name
	Name string `json:"name,omitempty"`

	// orcid
	Orcid string `json:"orcid,omitempty"`

	// redirect
	Redirect string `json:"redirect,omitempty"`

	// revision
	Revision string `json:"revision,omitempty"`

	// state
	// Required: true
	// Enum: [wip active redirect deleted]
	State *string `json:"state"`
}

// Validate validates this creator entity
func (m *CreatorEntity) Validate(formats strfmt.Registry) error {
	var res []error

	if err := m.validateIdent(formats); err != nil {
		res = append(res, err)
	}

	if err := m.validateState(formats); err != nil {
		res = append(res, err)
	}

	if len(res) > 0 {
		return errors.CompositeValidationError(res...)
	}
	return nil
}

func (m *CreatorEntity) validateIdent(formats strfmt.Registry) error {

	if err := validate.Required("ident", "body", m.Ident); err != nil {
		return err
	}

	return nil
}

var creatorEntityTypeStatePropEnum []interface{}

func init() {
	var res []string
	if err := json.Unmarshal([]byte(`["wip","active","redirect","deleted"]`), &res); err != nil {
		panic(err)
	}
	for _, v := range res {
		creatorEntityTypeStatePropEnum = append(creatorEntityTypeStatePropEnum, v)
	}
}

const (

	// CreatorEntityStateWip captures enum value "wip"
	CreatorEntityStateWip string = "wip"

	// CreatorEntityStateActive captures enum value "active"
	CreatorEntityStateActive string = "active"

	// CreatorEntityStateRedirect captures enum value "redirect"
	CreatorEntityStateRedirect string = "redirect"

	// CreatorEntityStateDeleted captures enum value "deleted"
	CreatorEntityStateDeleted string = "deleted"
)

// prop value enum
func (m *CreatorEntity) validateStateEnum(path, location string, value string) error {
	if err := validate.Enum(path, location, value, creatorEntityTypeStatePropEnum); err != nil {
		return err
	}
	return nil
}

func (m *CreatorEntity) validateState(formats strfmt.Registry) error {

	if err := validate.Required("state", "body", m.State); err != nil {
		return err
	}

	// value enum
	if err := m.validateStateEnum("state", "body", *m.State); err != nil {
		return err
	}

	return nil
}

// MarshalBinary interface implementation
func (m *CreatorEntity) MarshalBinary() ([]byte, error) {
	if m == nil {
		return nil, nil
	}
	return swag.WriteJSON(m)
}

// UnmarshalBinary interface implementation
func (m *CreatorEntity) UnmarshalBinary(b []byte) error {
	var res CreatorEntity
	if err := swag.ReadJSON(b, &res); err != nil {
		return err
	}
	*m = res
	return nil
}
